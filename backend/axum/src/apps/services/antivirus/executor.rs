//! The root helper: fixed programs and bounded requests, with quarantine metadata held only by root.
use super::scan_cache::{self, ScanCache, Stamp};
use super::types::*;
use crate::utils::now_ms;
use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::Read,
    os::{
        fd::AsRawFd,
        unix::{
            ffi::{OsStrExt, OsStringExt},
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        },
    },
    path::{Component, Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    process::Command,
    sync::mpsc,
};

const STATE: &str = "/var/lib/nas-os-antivirus";
const REQUEST: &str = "/var/lib/nas-os/antivirus.request.json";
const RESULT: &str = "/var/lib/nas-os/antivirus.result.json";
const DATA: &str = "/";
const MAX_DETECTIONS: usize = 1000;
/// The files found clean, by their stamp: see `scan_cache`.
const SCAN_CACHE: &str = "scan-cache.sqlite3";

fn failure(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn native_path(path: &str) -> PathBuf {
    #[cfg(test)]
    if let Some(path) = tests::path(path) {
        return path;
    }
    PathBuf::from(path)
}
fn program(name: &str) -> PathBuf {
    #[cfg(test)]
    if let Some(path) = tests::program(name) {
        return path;
    }
    PathBuf::from(name)
}
fn state_file(name: &str) -> PathBuf {
    native_path(STATE).join(name)
}

async fn persist(name: &str, bytes: &[u8]) -> Result<(), String> {
    let temporary = state_file(&format!("{name}.next"));
    tokio::fs::write(&temporary, bytes).await.map_err(failure)?;
    tokio::fs::rename(temporary, state_file(name))
        .await
        .map_err(failure)
}

async fn command(program: &str, arguments: &[&str]) -> Result<(), String> {
    let output = Command::new(self::program(program))
        .args(arguments)
        .output()
        .await
        .map_err(failure)?;
    if output.status.success() {
        Ok(())
    } else {
        let failure = failed(program, &output.stdout, &output.stderr);
        // Into the journal of the unit as an error, whole, beside the reason the launcher shows.
        eprintln!("<3>{failure}");
        Err(failure)
    }
}

/// Why a command failed: what it wrote on either stream (freshclam writes its errors on stdout
/// with `--stdout`), the end of it, where the error is.
fn failed(program: &str, stdout: &[u8], stderr: &[u8]) -> String {
    let written = [stderr, stdout]
        .iter()
        .map(|stream| String::from_utf8_lossy(stream).trim().to_owned())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if written.is_empty() {
        return format!("{program} failed without a message");
    }
    let skip = written.chars().count().saturating_sub(2000);
    format!(
        "{program} failed: {}",
        written.chars().skip(skip).collect::<String>()
    )
}

async fn publish(report: &mut Report) -> Result<(), String> {
    report.updated_at = now_ms();
    let bytes = serde_json::to_vec(&*report).map_err(failure)?;
    tokio::fs::write(state_file("state.next"), &bytes)
        .await
        .map_err(failure)?;
    tokio::fs::rename(state_file("state.next"), state_file("state.json"))
        .await
        .map_err(failure)?;
    publish_bytes(&bytes, RESULT).await
}

async fn publish_bytes(bytes: &[u8], target: &str) -> Result<(), String> {
    let temporary = state_file("result.next");
    tokio::fs::write(&temporary, bytes).await.map_err(failure)?;
    let uid = Command::new(program("id"))
        .args(["-u", "nas-os"])
        .output()
        .await
        .map_err(failure)?;
    let uid = String::from_utf8_lossy(&uid.stdout)
        .trim()
        .parse::<u32>()
        .map_err(failure)?;
    std::os::unix::fs::chown(&temporary, Some(uid), None).map_err(failure)?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600)).map_err(failure)?;
    tokio::fs::rename(temporary, native_path(target))
        .await
        .map_err(failure)
}

pub async fn run() -> Result<(), String> {
    // Never accept a caller-supplied request filename, program, command argument or quarantine path.
    let mut bytes = Vec::new();
    File::open(native_path(REQUEST))
        .map_err(failure)?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(failure)?;
    if bytes.len() > 65536 {
        return Err("antivirus request too large".into());
    }
    let job: Job = serde_json::from_slice(&bytes).map_err(failure)?;
    tokio::fs::create_dir_all(native_path(STATE))
        .await
        .map_err(failure)?;
    fs::set_permissions(native_path(STATE), fs::Permissions::from_mode(0o700)).map_err(failure)?;
    let mut report = load_report().await?;
    report.operation_error = None;
    let scanning = matches!(&job, Job::Scan { .. });
    let result = execute(job, &mut report).await;
    if let Err(reason) = &result {
        report.operation_error = Some(reason.clone());
        if scanning {
            report.error = Some(reason.clone());
        }
    }
    publish(&mut report).await?;
    result
}

/// Runs inside the OS lifecycle unit, before filesystems and networking stop.
pub async fn lifecycle(boot: bool) -> Result<(), String> {
    if !boot && !system_stopping().await? {
        return Ok(());
    }
    let bytes = match tokio::fs::read(state_file("settings.json")).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(failure(error)),
    };
    let settings: Settings = serde_json::from_slice(&bytes).map_err(failure)?;
    settings.validate().map_err(|error| error.1)?;
    let selected = if boot {
        &settings.boot_scan
    } else {
        &settings.shutdown_scan
    };
    if !settings.enabled || !selected.enabled {
        return Ok(());
    }
    if boot && !new_boot().await? {
        return Ok(());
    }
    let mut report = load_report().await?;
    // Reuse the storage helper so Explorer's bind mounts exist before a boot scan.
    let prepared = if boot {
        command("/usr/local/bin/nas-os-axum", &["--reconcile-storage"]).await
    } else {
        Ok(())
    };
    report.at = now_ms();
    report.kind = Some(selected.kind);
    report.scanned = 0;
    report.found = 0;
    let result = match prepared {
        Ok(()) => {
            scan(
                ScanRequest {
                    kind: selected.kind,
                    paths: selected.paths.clone(),
                    skip_unchanged: false,
                },
                settings.options(selected.kind).clone(),
                settings.auto_quarantine,
                &mut report,
            )
            .await
        }
        Err(reason) => Err(reason),
    };
    report.error = result.as_ref().err().cloned();
    report.operation_error = report.error.clone();
    publish(&mut report).await?;
    record_lifecycle_notice(&report).await?;
    // A failed boot scan is reported, while the unit remains armed for shutdown.
    if boot {
        Ok(())
    } else {
        result
    }
}

async fn system_stopping() -> Result<bool, String> {
    let output = Command::new(program("systemctl"))
        .arg("is-system-running")
        .output()
        .await
        .map_err(failure)?;
    Ok(String::from_utf8_lossy(&output.stdout).trim() == "stopping")
}

async fn new_boot() -> Result<bool, String> {
    let id = tokio::fs::read("/proc/sys/kernel/random/boot_id")
        .await
        .map_err(failure)?;
    if tokio::fs::read(state_file("boot-id")).await.ok().as_ref() == Some(&id) {
        return Ok(false);
    }
    tokio::fs::write(state_file("boot-id"), id)
        .await
        .map_err(failure)?;
    Ok(true)
}

async fn record_lifecycle_notice(report: &Report) -> Result<(), String> {
    let mut notices: Vec<LifecycleNotice> = tokio::fs::read(state_file("notices.json"))
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    notices.push(LifecycleNotice {
        at: report.at,
        found: report.found,
        scanned: report.scanned,
        error: report.error.clone(),
    });
    if notices.len() > 20 {
        notices.drain(..notices.len() - 20);
    }
    let bytes = serde_json::to_vec(&notices).map_err(failure)?;
    persist("notices.json", &bytes).await?;
    publish_bytes(&bytes, "/var/lib/nas-os/antivirus.notices.json").await
}

async fn load_report() -> Result<Report, String> {
    match tokio::fs::read(state_file("state.json")).await {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(failure),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Report::default()),
        Err(error) => Err(failure(error)),
    }
}

async fn execute(job: Job, report: &mut Report) -> Result<(), String> {
    match job {
        Job::Configure { settings } => {
            settings.validate().map_err(|error| error.1)?;
            persist(
                "settings.json",
                &serde_json::to_vec(&settings).map_err(failure)?,
            )
            .await?;
            // Arming the unit now must not count as booting the operating system.
            let boot = tokio::fs::read("/proc/sys/kernel/random/boot_id")
                .await
                .map_err(failure)?;
            tokio::fs::write(state_file("boot-id"), boot)
                .await
                .map_err(failure)?;
            if settings.enabled && (settings.boot_scan.enabled || settings.shutdown_scan.enabled) {
                command(
                    "systemctl",
                    &["enable", "nas-os-antivirus-lifecycle.service"],
                )
                .await?;
                command(
                    "systemctl",
                    &["start", "--no-block", "nas-os-antivirus-lifecycle.service"],
                )
                .await
            } else {
                command(
                    "systemctl",
                    &[
                        "disable",
                        "--now",
                        "--no-block",
                        "nas-os-antivirus-lifecycle.service",
                    ],
                )
                .await
            }
        }
        Job::Enable => {
            command("systemctl", &["stop", "clamav-freshclam.service"]).await?;
            let updated = command("freshclam", &["--stdout"]).await;
            if let Err(error) = updated {
                let _ = command(
                    "systemctl",
                    &["disable", "--now", "clamav-freshclam.service"],
                )
                .await;
                return Err(error);
            }
            command(
                "systemctl",
                &["enable", "--now", "clamav-freshclam.service"],
            )
            .await
        }
        Job::Resume { enabled: true } => {
            command(
                "systemctl",
                &["enable", "--now", "clamav-freshclam.service"],
            )
            .await
        }
        Job::Resume { enabled: false } | Job::Disable => {
            if let Ok(bytes) = tokio::fs::read(state_file("settings.json")).await {
                let mut settings: Settings = serde_json::from_slice(&bytes).map_err(failure)?;
                settings.enabled = false;
                persist(
                    "settings.json",
                    &serde_json::to_vec(&settings).map_err(failure)?,
                )
                .await?;
            }
            command(
                "systemctl",
                &[
                    "disable",
                    "--now",
                    "--no-block",
                    "nas-os-antivirus-lifecycle.service",
                ],
            )
            .await?;
            if Path::new("/usr/bin/freshclam").exists() {
                command(
                    "systemctl",
                    &["disable", "--now", "clamav-freshclam.service"],
                )
                .await?;
            }
            Ok(())
        }
        Job::Scan {
            request,
            options,
            auto_quarantine,
        } => scan(request, options, auto_quarantine, report).await,
        Job::Quarantine { id } => quarantine(&id, report).await,
        Job::Restore { id } => restore(&id, report),
        Job::Forget => {
            for suffix in ["", "-wal", "-shm"] {
                match fs::remove_file(state_file(&format!("{SCAN_CACHE}{suffix}"))) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(failure(error)),
                }
            }
            report.cache_files = 0;
            report.cache_bytes = 0;
            Ok(())
        }
        Job::Delete { id } => {
            if !identifier(&id) {
                return Err("invalid quarantine identifier".into());
            }
            let index = report
                .quarantine
                .iter()
                .position(|entry| entry.detection.id == id)
                .ok_or("quarantine entry missing")?;
            fs::remove_file(state_file(&id)).map_err(failure)?;
            report.quarantine.remove(index);
            Ok(())
        }
    }
}

/// Open each parent directory without following links. The fd anchors later moves even if a
/// storage user renames directories while the operation runs. Linux O_DIRECTORY and O_NOFOLLOW.
fn parent(path: &str) -> Result<(File, PathBuf), String> {
    if !valid_path(path) {
        return Err("invalid file path".into());
    }
    if excluded(path) {
        return Err("the requested path is reserved or virtual".into());
    }
    let relative = Path::new(path)
        .strip_prefix(DATA)
        .map_err(|_| "an absolute path is required")?;
    let name = relative.file_name().ok_or("a file is required")?;
    let mut directory = OpenOptions::new()
        .read(true)
        .custom_flags(0x10000 | 0x20000)
        .open(DATA)
        .map_err(failure)?;
    for component in relative
        .parent()
        .ok_or("a parent directory is required")?
        .components()
    {
        let Component::Normal(component) = component else {
            return Err("invalid parent directory".into());
        };
        directory = OpenOptions::new()
            .read(true)
            .custom_flags(0x10000 | 0x20000)
            .open(PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd())).join(component))
            .map_err(failure)?;
    }
    let anchored = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd())).join(name);
    Ok((directory, anchored))
}

async fn quarantine(id: &str, report: &mut Report) -> Result<(), String> {
    if !identifier(id) {
        return Err("invalid detection identifier".into());
    }
    let index = report
        .detections
        .iter()
        .position(|entry| entry.id == id)
        .ok_or("detection missing")?;
    let detection = report.detections[index].clone();
    let (_directory, source) = parent(&detection.path)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(0x20000)
        .open(&source)
        .map_err(failure)?;
    let metadata = file.metadata().map_err(failure)?;
    if !metadata.is_file()
        || metadata.dev() != detection.device
        || metadata.ino() != detection.inode
    {
        return Err("the detected file has changed, scan it again".into());
    }
    // Filenames can contain newlines, so a line in clamscan's text output alone is never enough
    // to move a file. Confirm the finding on this opened inode using the scanner's exit status.
    let descriptor = format!("/proc/{}/fd/{}", std::process::id(), file.as_raw_fd());
    let verified = Command::new(program("clamscan"))
        .args(["--no-summary", "--infected", "--follow-file-symlinks=1"])
        .arg(format!("--max-filesize={}M", detection.options.max_file_mb))
        .arg(format!(
            "--detect-pua={}",
            if detection.options.detect_pua {
                "yes"
            } else {
                "no"
            }
        ))
        .arg(format!(
            "--scan-archive={}",
            if detection.options.archives {
                "yes"
            } else {
                "no"
            }
        ))
        .arg("--")
        .arg(descriptor)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(failure)?;
    if verified.code() != Some(1) {
        return Err("the file is no longer detected as a threat, scan it again".into());
    }
    let target = state_file(id);
    // Copy the opened file into a private root directory. Never change permissions on the original
    // inode: another hard link may exist. The original is removed only after a successful copy.
    let mut destination = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&target)
        .map_err(failure)?;
    let copied = std::io::copy(&mut &file, &mut destination).and_then(|_| destination.sync_all());
    if let Err(error) = copied {
        let _ = fs::remove_file(&target);
        return Err(failure(error));
    }
    let current = fs::symlink_metadata(&source).map_err(failure)?;
    if current.dev() != detection.device || current.ino() != detection.inode {
        let _ = fs::remove_file(target);
        return Err("the detected file changed during quarantine".into());
    }
    report.quarantine.push(QuarantineEntry {
        detection,
        at: now_ms(),
        mode: metadata.mode() & 0o777,
        uid: metadata.uid(),
        gid: metadata.gid(),
    });
    // Persist recovery metadata before removing the source. A stopped helper may leave a copy,
    // but cannot leave the only surviving file without its original location.
    publish(report).await?;
    fs::remove_file(source).map_err(failure)?;
    report.detections.remove(index);
    Ok(())
}

fn restore(id: &str, report: &mut Report) -> Result<(), String> {
    if !identifier(id) {
        return Err("invalid quarantine identifier".into());
    }
    let index = report
        .quarantine
        .iter()
        .position(|entry| entry.detection.id == id)
        .ok_or("quarantine entry missing")?;
    let entry = &report.quarantine[index];
    let (_directory, target) = parent(&entry.detection.path)?;
    // create_new refuses existing files and symlinks. Directory fds prevent traversal races.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&target)
        .map_err(failure)?;
    let mut source = File::open(state_file(id)).map_err(failure)?;
    let restored = std::io::copy(&mut source, &mut file).and_then(|_| file.sync_all());
    if let Err(error) = restored {
        let _ = fs::remove_file(&target);
        return Err(failure(error));
    }
    let descriptor = format!("/proc/self/fd/{}", file.as_raw_fd());
    std::os::unix::fs::chown(descriptor, Some(entry.uid), Some(entry.gid)).map_err(failure)?;
    file.set_permissions(fs::Permissions::from_mode(entry.mode))
        .map_err(failure)?;
    fs::remove_file(state_file(id)).map_err(failure)?;
    report.quarantine.remove(index);
    Ok(())
}

fn excluded(path: &str) -> bool {
    [
        "/proc",
        "/sys",
        "/dev",
        "/run",
        STATE,
        "/var/lib/nas-os",
        "/var/lib/clamav",
        "/srv/nas-volumes",
    ]
    .iter()
    .any(|root| Path::new(path).starts_with(root))
}

async fn scan(
    request: ScanRequest,
    options: ScanOptions,
    auto_quarantine: bool,
    report: &mut Report,
) -> Result<(), String> {
    report.at = now_ms();
    report.kind = Some(request.kind);
    report.scanned = 0;
    report.found = 0;
    report.skipped = 0;
    report.planned = 0;
    report.error = None;
    let incremental = request.kind == ScanKind::Quick
        || (request.kind == ScanKind::Custom && request.skip_unchanged);
    let paths = scan_paths(request, &options).await?;
    let cache = Clean::open(&options).await;

    let mut command = scan_command(&options);
    match (&cache, incremental) {
        (Some(clean), true) => {
            let planned = plan(clean, &paths, &options).await?;
            report.skipped = planned.skipped;
            report.planned = planned.files;
            if planned.files == 0 {
                clean.finish(report).await;
                publish(report).await?;
                return Ok(());
            }
            command
                .arg(format!("--file-list={}", planned.list.display()))
                .arg("--")
                .args(&planned.by_name);
        }
        _ => {
            command.arg("--").args(&paths);
        }
    }

    publish(report).await?;
    // Keep unresolved detections across scans. A detection for the same inode is updated below.
    let stderr = File::create(state_file("scan.error")).map_err(failure)?;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(stderr)
        .kill_on_drop(true)
        .spawn()
        .map_err(failure)?;
    let stdout = child.stdout.take().ok_or("scanner output unavailable")?;
    let mut lines = BufReader::new(stdout).lines();
    let mut last_publish = Instant::now();
    let mut scan_detected = 0;
    let mut scan_errors = Vec::new();
    let mut found_clean = Vec::new();
    while let Some(line) = lines.next_line().await.map_err(failure)? {
        if let Some(clean) = &cache {
            if let Some(path) = line.strip_suffix(": OK") {
                if in_scan_scope(path, &paths, &options.exclusions) {
                    found_clean.push(PathBuf::from(path));
                }
                if found_clean.len() >= scan_cache::BATCH {
                    clean.record(std::mem::take(&mut found_clean)).await;
                }
            } else if let Some((path, _)) = line
                .strip_suffix(" FOUND")
                .and_then(|found| found.rsplit_once(": "))
            {
                clean.forget(OsStr::new(path)).await;
            }
        }
        if !read_scan_line(
            &line,
            &options,
            &paths,
            report,
            &mut scan_detected,
            &mut scan_errors,
        )? {
            continue;
        }
        if last_publish.elapsed() > Duration::from_secs(2) {
            publish(report).await?;
            last_publish = Instant::now();
        }
    }
    let status = child.wait().await.map_err(failure)?;
    if let Some(clean) = &cache {
        clean.record(found_clean).await;
        clean.finish(report).await;
    }
    if !matches!(status.code(), Some(0 | 1)) {
        let bytes = tokio::fs::read(state_file("scan.error"))
            .await
            .unwrap_or_default();
        return Err(format!(
            "ClamAV scan failed: {}",
            String::from_utf8_lossy(&bytes[..bytes.len().min(2000)])
        ));
    }
    if !scan_errors.is_empty() {
        return Err(format!("scan incomplete: {}", scan_errors.join("; ")));
    }
    if auto_quarantine {
        let ids: Vec<_> = report
            .detections
            .iter()
            .filter(|entry| !excluded(&entry.path))
            .map(|entry| entry.id.clone())
            .collect();
        for id in ids {
            quarantine(&id, report).await?;
            publish(report).await?;
        }
    }
    if status.code() == Some(1) && scan_detected == 0 {
        return Err("ClamAV reported threats but the findings could not be read".into());
    }
    Ok(())
}

/// The archive of the files found clean during one scan, with the options and the moment it
/// started. Its failures go to the journal and never fail the scan.
struct Clean {
    cache: ScanCache,
    options: String,
    started_ms: i64,
    started_ns: i128,
}

impl Clean {
    async fn open(options: &ScanOptions) -> Option<Self> {
        let started_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i128::try_from(elapsed.as_nanos()).unwrap_or(i128::MAX)
            });
        match ScanCache::open(&state_file(SCAN_CACHE)).await {
            Ok(cache) => Some(Self {
                cache,
                options: scan_cache::fingerprint(options),
                started_ms: now_ms(),
                started_ns,
            }),
            Err(error) => {
                eprintln!("<4>the files found clean are not used: {error}");
                None
            }
        }
    }

    /// Files ClamAV said were clean. A file changed since the scan started is not recorded: what
    /// it holds now was not scanned.
    async fn record(&self, paths: Vec<PathBuf>) {
        let started = self.started_ns;
        let files = tokio::task::spawn_blocking(move || {
            paths
                .into_iter()
                .filter_map(|path| {
                    let metadata = fs::symlink_metadata(&path).ok()?;
                    let stamp = Stamp::of(&metadata);
                    (metadata.is_file() && stamp.changed_ns <= started)
                        .then(|| (path.into_os_string().into_vec(), stamp))
                })
                .collect::<Vec<_>>()
        })
        .await
        .unwrap_or_default();
        if let Err(error) = self
            .cache
            .record(&files, &self.options, self.started_ms)
            .await
        {
            eprintln!("<4>the files found clean were not recorded: {error}");
        }
    }

    async fn forget(&self, path: &OsStr) {
        if let Err(error) = self.cache.forget(path).await {
            eprintln!("<4>a file found infected is still recorded clean: {error}");
        }
    }

    /// Drops the results too old, and says in the report what the archive holds now.
    async fn finish(&self, report: &mut Report) {
        if let Err(error) = self.cache.prune(self.started_ms).await {
            eprintln!("<4>the old clean files were not pruned: {error}");
        }
        report.cache_files = self.cache.count().await.unwrap_or(report.cache_files);
        report.cache_bytes = ["", "-wal", "-shm"]
            .iter()
            .filter_map(|suffix| fs::metadata(state_file(&format!("{SCAN_CACHE}{suffix}"))).ok())
            .map(|metadata| metadata.len())
            .sum();
    }
}

/// What an incremental scan sends to ClamAV: the list of the files to scan (one per line), those
/// whose name a list cannot hold (a line break in it), and how many were left out.
struct Planned {
    list: PathBuf,
    by_name: Vec<PathBuf>,
    files: u64,
    skipped: u64,
}

/// Goes through the folders as ClamAV would (no links followed, the same exclusions) and keeps the
/// files that are new, changed, or found clean more than 30 days ago.
async fn plan(clean: &Clean, roots: &[String], options: &ScanOptions) -> Result<Planned, String> {
    let list = state_file("scan.list");
    let mut writer = BufWriter::new(tokio::fs::File::create(&list).await.map_err(failure)?);
    let (sender, mut receiver) = mpsc::channel(4);
    let roots: Vec<PathBuf> = roots.iter().map(PathBuf::from).collect();
    let exclusions = options.exclusions.clone();
    let walker = tokio::task::spawn_blocking(move || walk(roots, &exclusions, &sender));
    let mut planned = Planned {
        list: list.clone(),
        by_name: Vec::new(),
        files: 0,
        skipped: 0,
    };
    while let Some(batch) = receiver.recv().await {
        // An archive that cannot answer leaves nothing out.
        let known = clean
            .cache
            .unchanged(&batch, &clean.options, clean.started_ms)
            .await
            .unwrap_or_default();
        for (path, _) in batch {
            if known.contains(&path) {
                planned.skipped += 1;
            } else if path.contains(&b'\n') {
                planned
                    .by_name
                    .push(PathBuf::from(OsStr::from_bytes(&path)));
                planned.files += 1;
            } else {
                writer.write_all(&path).await.map_err(failure)?;
                writer.write_all(b"\n").await.map_err(failure)?;
                planned.files += 1;
            }
        }
    }
    walker.await.map_err(failure)?;
    writer.flush().await.map_err(failure)?;
    Ok(planned)
}

/// The regular files under the roots, in batches, without following links and leaving out what
/// the scan leaves out.
fn walk(roots: Vec<PathBuf>, exclusions: &[String], sender: &mpsc::Sender<Vec<(Vec<u8>, Stamp)>>) {
    let mut batch = Vec::with_capacity(scan_cache::BATCH);
    let mut pending = roots;
    while let Some(path) = pending.pop() {
        let text = path.to_string_lossy();
        if excluded(&text)
            || exclusions
                .iter()
                .any(|excluded| Path::new(text.as_ref()).starts_with(excluded))
        {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_dir() {
            if let Ok(entries) = fs::read_dir(&path) {
                pending.extend(entries.flatten().map(|entry| entry.path()));
            }
        } else if metadata.is_file() {
            batch.push((path.into_os_string().into_vec(), Stamp::of(&metadata)));
            if batch.len() >= scan_cache::BATCH
                && sender.blocking_send(std::mem::take(&mut batch)).is_err()
            {
                return;
            }
        }
    }
    if !batch.is_empty() {
        let _ = sender.blocking_send(batch);
    }
}

async fn scan_paths(request: ScanRequest, options: &ScanOptions) -> Result<Vec<String>, String> {
    request.validate().map_err(|error| error.1)?;
    let config = Settings {
        custom: options.clone(),
        ..Settings::default()
    };
    config.validate().map_err(|error| error.1)?;
    let mut paths = match request.kind {
        ScanKind::Full => vec!["/".to_owned()],
        ScanKind::Quick => {
            if options.paths.is_empty() {
                vec!["/home".into(), "/tmp".into(), "/var/tmp".into()]
            } else {
                options.paths.clone()
            }
        }
        ScanKind::File | ScanKind::Custom => request.paths,
    };
    for path in &mut paths {
        let canonical = tokio::fs::canonicalize(&*path).await.map_err(failure)?;
        if excluded(&canonical.to_string_lossy()) {
            return Err("the requested path is reserved or virtual".into());
        }
        if request.kind == ScanKind::File && !canonical.is_file() {
            return Err("file scan requires a regular file".into());
        }
        *path = canonical
            .to_str()
            .ok_or("invalid scan path encoding")?
            .to_owned();
    }

    Ok(paths)
}

fn scan_command(options: &ScanOptions) -> Command {
    let mut command = Command::new(program("clamscan"));
    command.args(["--recursive=yes", "--follow-dir-symlinks=0", "--follow-file-symlinks=0", "--cross-fs=yes", "--stdout"])
        .arg(format!("--scan-archive={}", if options.archives { "yes" } else { "no" }))
        .arg(format!("--detect-pua={}", if options.detect_pua { "yes" } else { "no" }))
        .arg(format!("--max-filesize={}M", options.max_file_mb))
        .arg("--exclude-dir=^/(proc|sys|dev|run|srv/nas-volumes|var/lib/(nas-os|nas-os-antivirus|clamav))(/|$)");
    for path in &options.exclusions {
        // Escape literal paths instead of treating user input as ClamAV regular expressions.
        let escaped: String = path
            .chars()
            .flat_map(|character| {
                if ".+*?()[]{}^$|\\".contains(character) {
                    vec!['\\', character]
                } else {
                    vec![character]
                }
            })
            .collect();
        command
            .arg(format!("--exclude=^{escaped}(/|$)"))
            .arg(format!("--exclude-dir=^{escaped}(/|$)"));
    }
    command
}

fn in_scan_scope(path: &str, paths: &[String], exclusions: &[String]) -> bool {
    !excluded(path)
        && paths.iter().any(|root| Path::new(path).starts_with(root))
        && !exclusions
            .iter()
            .any(|excluded| Path::new(path).starts_with(excluded))
}

/// Returns false for findings outside the requested scope, so they also skip progress publication.
fn read_scan_line(
    line: &str,
    options: &ScanOptions,
    paths: &[String],
    report: &mut Report,
    scan_detected: &mut usize,
    scan_errors: &mut Vec<String>,
) -> Result<bool, String> {
    if line.ends_with(" ERROR") && scan_errors.len() < 10 {
        scan_errors.push(line.chars().take(500).collect::<String>());
    }
    if line.ends_with(": OK") {
        report.scanned += 1;
    }
    if let Some(mut detection) = detection(line, report.at, *scan_detected) {
        if !in_scan_scope(&detection.path, paths, &options.exclusions) {
            return Ok(false);
        }
        detection.options = options.clone();
        *scan_detected += 1;
        report.found += 1;
        if report.detections.len() >= MAX_DETECTIONS {
            return Err("too many detections, scan a smaller scope".into());
        }
        report
            .detections
            .retain(|previous| previous.path != detection.path);
        report.detections.push(detection);
    }
    if let Some(count) = line.strip_prefix("Scanned files: ") {
        report.scanned = count.parse().unwrap_or(report.scanned);
    }
    Ok(true)
}

fn detection(line: &str, at: i64, index: usize) -> Option<Detection> {
    let (path, threat) = line.strip_suffix(" FOUND")?.rsplit_once(": ")?;
    if !valid_path(path) {
        return None;
    }
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    Some(Detection {
        id: format!("{at}-{index}"),
        path: path.into(),
        threat: threat.into(),
        device: metadata.dev(),
        inode: metadata.ino(),
        options: ScanOptions::default(),
    })
}

#[cfg(test)]
#[path = "../../../../../tests/apps/services/antivirus/executor.rs"]
mod tests;
