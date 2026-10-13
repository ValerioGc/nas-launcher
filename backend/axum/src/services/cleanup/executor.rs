//! The root side of the cleanup: `nas-os-axum --cleanup-job`, run by `nas-os-cleanup.service` on
//! the request the launcher wrote beside its database. Fixed paths and fixed programs, never a
//! link followed, never another filesystem crossed. A cleanup measures the item before and after,
//! and says what it freed and what went.

use super::{Action, Answer, Category, Disk, Item, Options, Outcome, Overview, Request};
use std::{
    collections::HashSet,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use tokio::process::Command;

pub const REQUEST: &str = "/var/lib/nas-os/cleanup.request.json";
pub const RESULT: &str = "/var/lib/nas-os/cleanup.result.json";
/// The entries an outcome lists: the others are counted.
const SHOWN: usize = 200;
/// What an update left is taken only past this age: an update may still be under way before.
const LEFT_FOR: Duration = Duration::from_secs(24 * 60 * 60);
const ARCHIVES: &str = "/var/cache/apt/archives";
const LOGS: &str = "/var/log";
const JOURNAL: &str = "/var/log/journal";
const TEMPORARY: [&str; 2] = ["/tmp", "/var/tmp"];
/// What lives in the temporary folders for programs still running: never touched.
const KEPT_IN_TEMPORARY: [&str; 7] = [
    "systemd-private-",
    ".X11-unix",
    ".ICE-unix",
    ".XIM-unix",
    ".font-unix",
    ".Test-unix",
    "snap-private-tmp",
];
const STATE: &str = "/var/lib/nas-os";
const BACKUPS: &str = "/var/lib/nas-os/backups";
const APPS: &str = "/var/lib/nas-os/apps";
const REGISTRY: &str = "/etc/nas-os/storage-volumes.tsv";
const UPDATE_UNIT: &str = "nas-os-update.service";

/// The machine the job works on: `/` and the programs of the system, or a folder and stand-in
/// programs in the tests.
pub struct Machine {
    root: PathBuf,
    bin: Option<PathBuf>,
    /// The time the ages are counted from: now, or later in the tests.
    now: Option<SystemTime>,
}

impl Machine {
    pub fn native() -> Self {
        Self::new(PathBuf::from("/"), None)
    }

    pub fn new(root: PathBuf, bin: Option<PathBuf>) -> Self {
        Self {
            root,
            bin,
            now: None,
        }
    }

    /// The same machine, with the ages counted from `now`.
    pub fn at_time(self, now: SystemTime) -> Self {
        Self {
            now: Some(now),
            ..self
        }
    }

    fn now(&self) -> SystemTime {
        self.now.unwrap_or_else(SystemTime::now)
    }

    pub fn at(&self, path: &str) -> PathBuf {
        self.root.join(path.trim_start_matches('/'))
    }

    fn program(&self, name: &str) -> PathBuf {
        self.bin
            .as_ref()
            .map_or_else(|| PathBuf::from(name), |bin| bin.join(name))
    }

    /// What a program wrote, when it succeeded.
    async fn output(&self, program: &str, arguments: &[&str]) -> Result<String, String> {
        let output = Command::new(self.program(program))
            .args(arguments)
            .env("DEBIAN_FRONTEND", "noninteractive")
            .env("LC_ALL", "C")
            .output()
            .await
            .map_err(|error| format!("{program} is not available: {error}"))?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        let written = String::from_utf8_lossy(&output.stderr);
        let tail: String = written
            .chars()
            .skip(written.chars().count().saturating_sub(1000))
            .collect();
        eprintln!("<3>{program} failed: {written}");
        Err(format!("{program} failed: {}", tail.trim()))
    }

    /// Whether an update of the NAS is under way: its backups and leftovers are then its own.
    async fn updating(&self) -> bool {
        self.output("systemctl", &["is-active", "--quiet", UPDATE_UNIT])
            .await
            .is_ok()
    }
}

/// What an item holds: its size, what a person reads, and what the cleanup removes.
#[derive(Debug, Default)]
struct Plan {
    bytes: u64,
    entries: Vec<String>,
    files: Vec<PathBuf>,
    packages: Vec<String>,
}

impl Plan {
    fn of_files(machine: &Machine, files: Vec<(PathBuf, u64)>) -> Self {
        let mut plan = Self::default();
        for (path, size) in files {
            plan.bytes += size;
            plan.entries.push(shown(machine, &path));
            plan.files.push(path);
        }
        plan
    }

    fn outcome(&self, item: Item) -> Outcome {
        Outcome {
            item: Some(item),
            bytes: self.bytes,
            entries: self.entries.iter().take(SHOWN).cloned().collect(),
            more: self.entries.len().saturating_sub(SHOWN) as u64,
        }
    }
}

/// A path as it is on the machine, without the folder of the tests.
fn shown(machine: &Machine, path: &Path) -> String {
    let relative = path.strip_prefix(&machine.root).unwrap_or(path);
    format!("/{}", relative.to_string_lossy().trim_start_matches('/'))
}

fn older_than(metadata: &fs::Metadata, age: Duration, now: SystemTime) -> bool {
    let Some(limit) = now.checked_sub(age) else {
        return false;
    };
    let changed = SystemTime::UNIX_EPOCH
        + Duration::from_secs(u64::try_from(metadata.ctime()).unwrap_or_default());
    metadata.modified().is_ok_and(|modified| modified < limit) && changed < limit
}

/// The files under a folder that `keep` accepts, with their size: links are not followed and
/// another filesystem is not crossed. A link counts as a file of its own.
fn files_under(
    start: &Path,
    skip: &dyn Fn(&Path) -> bool,
    keep: &dyn Fn(&Path, &fs::Metadata) -> bool,
) -> Vec<(PathBuf, u64)> {
    let Ok(top) = fs::symlink_metadata(start) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut folders = vec![start.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if skip(&path) {
                continue;
            }
            if metadata.is_dir() {
                if metadata.dev() == top.dev() {
                    folders.push(path);
                }
            } else if (metadata.is_file() || metadata.file_type().is_symlink())
                && keep(&path, &metadata)
            {
                found.push((path, metadata.len()));
            }
        }
    }
    found.sort();
    found
}

/// The room a folder takes, on its own filesystem.
fn size_of(path: &Path) -> u64 {
    files_under(path, &|_| false, &|_, _| true)
        .iter()
        .map(|(_, size)| size)
        .sum()
}

/// Whether a log is a rotated one: `syslog.1`, `syslog.2.gz`, `messages.old`.
fn rotated(name: &str) -> bool {
    let last = name.rsplit('.').next().unwrap_or_default();
    name.contains('.')
        && (matches!(last, "gz" | "xz" | "bz2" | "zst" | "old")
            || (!last.is_empty() && last.chars().all(|c| c.is_ascii_digit())))
}

/// `1.2G`, `512.0M`, `48K` as `journalctl --disk-usage` writes them, in bytes.
pub fn parse_size(text: &str) -> Option<u64> {
    let text = text.trim();
    let (number, unit) = text
        .find(|c: char| c.is_ascii_alphabetic())
        .map_or((text, ""), |at| text.split_at(at));
    let number: f64 = number.trim().parse().ok()?;
    let power = match unit.trim().chars().next().map(|c| c.to_ascii_uppercase()) {
        None | Some('B') => 0,
        Some('K') => 1,
        Some('M') => 2,
        Some('G') => 3,
        Some('T') => 4,
        _ => return None,
    };
    Some((number * 1024f64.powi(power)) as u64)
}

/// The size `journalctl --disk-usage` reports.
pub fn journal_usage(text: &str) -> Option<u64> {
    let (_, after) = text.split_once("take up ")?;
    parse_size(after.split_whitespace().next()?)
}

/// The packages apt would take away in a simulation: `Remv name [version]`, `Del name version`.
pub fn simulated(text: &str, verb: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| line.strip_prefix(verb)?.strip_prefix(' '))
        .filter_map(|rest| {
            let mut words = rest.split_whitespace();
            let name = words.next()?.to_owned();
            let version = words
                .next()
                .unwrap_or_default()
                .trim_matches(|c| c == '[' || c == ']')
                .to_owned();
            Some((name, version))
        })
        .collect()
}

/// The room the packages take once installed, in bytes.
async fn installed_size(machine: &Machine, packages: &[String]) -> u64 {
    if packages.is_empty() {
        return 0;
    }
    let mut arguments = vec!["-W", "-f=${Installed-Size}\\n"];
    arguments.extend(packages.iter().map(String::as_str));
    machine
        .output("dpkg-query", &arguments)
        .await
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.trim().parse::<u64>().ok())
        .sum::<u64>()
        * 1024
}

fn of_packages(packages: Vec<String>, bytes: u64) -> Plan {
    Plan {
        bytes,
        entries: packages.clone(),
        files: Vec::new(),
        packages,
    }
}

/// The kernels to remove: all but the one running, any newer one (it starts at the next boot) and
/// the newest one before it. Nothing when the running one is not a package.
pub async fn old_kernels(machine: &Machine) -> Result<Vec<(String, u64)>, String> {
    let running = machine.output("uname", &["-r"]).await?;
    let running = format!("linux-image-{}", running.trim());
    let listed = machine
        .output(
            "dpkg-query",
            &[
                "-W",
                "-f=${db:Status-Abbrev} ${Package} ${Version} ${Installed-Size}\\n",
                "linux-image-*",
            ],
        )
        .await
        .unwrap_or_default();
    let mut kernels: Vec<(String, String, u64)> = listed
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            (words.next()? == "ii").then_some(())?;
            let name = words.next()?;
            let version = words.next()?;
            let size = words.next().and_then(|size| size.parse().ok()).unwrap_or(0);
            name.strip_prefix("linux-image-")?
                .starts_with(|c: char| c.is_ascii_digit())
                .then(|| (name.to_owned(), version.to_owned(), size * 1024))
        })
        .collect();
    let Some(current) = kernels
        .iter()
        .find(|(name, _, _)| *name == running)
        .map(|(_, version, _)| version.clone())
    else {
        return Ok(Vec::new());
    };
    // Newest first, by the order of dpkg.
    let mut ordered: Vec<(String, String, u64)> = Vec::new();
    for kernel in kernels.drain(..) {
        let mut at = ordered.len();
        for (index, other) in ordered.iter().enumerate() {
            if newer(machine, &kernel.1, &other.1).await {
                at = index;
                break;
            }
        }
        ordered.insert(at, kernel);
    }
    let mut older_kept = false;
    let mut removed = Vec::new();
    for (name, version, size) in ordered {
        if name == running || version == current || newer(machine, &version, &current).await {
            continue;
        }
        if !older_kept {
            older_kept = true;
            continue;
        }
        removed.push((name, size));
    }
    Ok(removed)
}

async fn newer(machine: &Machine, version: &str, than: &str) -> bool {
    machine
        .output("dpkg", &["--compare-versions", version, "gt", than])
        .await
        .is_ok()
}

/// Files older than the age in the temporary folders, but what programs keep there.
fn temporary_files(machine: &Machine, days: u16) -> Vec<(PathBuf, u64)> {
    let age = Duration::from_secs(u64::from(days) * 24 * 60 * 60);
    let now = machine.now();
    TEMPORARY
        .iter()
        .flat_map(|folder| {
            let top = machine.at(folder);
            let kept = top.clone();
            files_under(
                &top,
                &move |path| {
                    path.parent() == Some(kept.as_path())
                        && path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| {
                                KEPT_IN_TEMPORARY
                                    .iter()
                                    .any(|prefix| name.starts_with(prefix))
                            })
                },
                &move |_, metadata| older_than(metadata, age, now),
            )
        })
        .collect()
}

/// The backups of the updates but the newest: it is the way back to the version before.
fn old_backups(machine: &Machine) -> Vec<(PathBuf, u64)> {
    let mut backups: Vec<(PathBuf, u64)> = fs::read_dir(machine.at(BACKUPS))
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tar.gz"))
                .filter_map(|entry| {
                    let metadata = fs::symlink_metadata(entry.path()).ok()?;
                    metadata.is_file().then(|| (entry.path(), metadata.len()))
                })
                .collect()
        })
        .unwrap_or_default();
    backups.sort();
    backups.pop();
    backups
}

/// Releases downloaded and versions of applications set aside by an update that did not finish.
fn leftovers(machine: &Machine) -> Vec<(PathBuf, u64)> {
    let now = machine.now();
    let mut found: Vec<(PathBuf, u64)> = ["wizard", "gateway", "launcher"]
        .iter()
        .map(|component| machine.at(&format!("{STATE}/{component}-update.tar")))
        .filter_map(|path| {
            let metadata = fs::symlink_metadata(&path).ok()?;
            (metadata.is_file() && older_than(&metadata, LEFT_FOR, now))
                .then_some((path, metadata.len()))
        })
        .collect();
    if let Ok(entries) = fs::read_dir(machine.at(APPS)) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let set_aside = name.starts_with('.')
                && (name.ends_with(".old") || name.ends_with(".new"))
                && name.len() > 5;
            let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if set_aside && metadata.is_dir() && older_than(&metadata, LEFT_FOR, now) {
                found.push((entry.path(), size_of(&entry.path())));
            }
        }
    }
    found.sort();
    found
}

/// What an item holds now.
async fn plan(machine: &Machine, item: Item, options: Options) -> Result<Plan, String> {
    Ok(match item {
        Item::AptArchives => {
            let archives = machine.at(ARCHIVES);
            Plan::of_files(
                machine,
                files_under(&archives, &|_| false, &|path, _| {
                    path.extension().is_some_and(|extension| extension == "deb")
                        || path
                            .parent()
                            .is_some_and(|parent| parent.ends_with("partial"))
                }),
            )
        }
        Item::AptObsolete => {
            let text = machine.output("apt-get", &["-s", "autoclean"]).await?;
            let obsolete = simulated(&text, "Del");
            let names: HashSet<String> = obsolete
                .iter()
                .map(|(name, version)| format!("{name}_{}_", version.replace(':', "%3a")))
                .collect();
            let files = files_under(&machine.at(ARCHIVES), &|_| false, &|path, _| {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                names.iter().any(|prefix| name.starts_with(prefix.as_str()))
            });
            Plan::of_files(machine, files)
        }
        Item::AptOrphans => {
            let text = machine.output("apt-get", &["-s", "autoremove"]).await?;
            let packages: Vec<String> = simulated(&text, "Remv")
                .into_iter()
                .map(|(name, _)| name)
                .collect();
            let bytes = installed_size(machine, &packages).await;
            of_packages(packages, bytes)
        }
        Item::AptConfigs => {
            let text = machine
                .output(
                    "dpkg-query",
                    &["-W", "-f=${db:Status-Abbrev} ${Package}\\n"],
                )
                .await?;
            let packages = text
                .lines()
                .filter_map(|line| line.strip_prefix("rc "))
                .map(|name| name.trim().to_owned())
                .filter(|name| !name.is_empty())
                .collect();
            of_packages(packages, 0)
        }
        Item::OldKernels => {
            let kernels = old_kernels(machine).await?;
            let bytes = kernels.iter().map(|(_, size)| size).sum();
            of_packages(kernels.into_iter().map(|(name, _)| name).collect(), bytes)
        }
        Item::RotatedLogs => {
            let journal = machine.at(JOURNAL);
            Plan::of_files(
                machine,
                files_under(
                    &machine.at(LOGS),
                    &move |path| path == journal,
                    &|path, metadata| {
                        metadata.is_file()
                            && path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .is_some_and(rotated)
                    },
                ),
            )
        }
        Item::Journal => {
            let text = machine.output("journalctl", &["--disk-usage"]).await?;
            let used = journal_usage(&text).ok_or("the size of the journal is unknown")?;
            let limit = u64::from(options.journal_mb) * 1024 * 1024;
            Plan {
                bytes: used.saturating_sub(limit),
                entries: vec![format!("{used} B")],
                ..Plan::default()
            }
        }
        Item::Temporary => {
            Plan::of_files(machine, temporary_files(machine, options.temporary_days))
        }
        Item::NasBackups => Plan::of_files(machine, old_backups(machine)),
        Item::NasLeftovers => Plan::of_files(machine, leftovers(machine)),
    })
}

/// Removes what a plan holds, by the fixed command of its item or file by file.
async fn remove(
    machine: &Machine,
    item: Item,
    plan: &Plan,
    options: Options,
) -> Result<(), String> {
    let packages: Vec<&str> = plan.packages.iter().map(String::as_str).collect();
    match item {
        Item::AptArchives => machine.output("apt-get", &["clean"]).await.map(drop),
        Item::AptObsolete => machine.output("apt-get", &["autoclean"]).await.map(drop),
        Item::AptOrphans if !packages.is_empty() => machine
            .output("apt-get", &["-y", "autoremove"])
            .await
            .map(drop),
        Item::AptConfigs if !packages.is_empty() => {
            let mut arguments = vec!["--purge"];
            arguments.extend(packages);
            machine.output("dpkg", &arguments).await.map(drop)
        }
        Item::OldKernels if !packages.is_empty() => {
            let mut arguments = vec!["-y", "purge"];
            arguments.extend(packages);
            machine.output("apt-get", &arguments).await.map(drop)
        }
        Item::Journal => machine
            .output(
                "journalctl",
                &[&format!("--vacuum-size={}M", options.journal_mb)],
            )
            .await
            .map(drop),
        Item::NasBackups | Item::NasLeftovers if machine.updating().await => {
            Err("an update of the NAS is under way".into())
        }
        Item::RotatedLogs | Item::Temporary | Item::NasBackups | Item::NasLeftovers => {
            for path in &plan.files {
                let removed = match fs::symlink_metadata(path) {
                    Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
                    Ok(_) => fs::remove_file(path),
                    Err(_) => Ok(()),
                };
                if let Err(error) = removed {
                    eprintln!("<4>{} was not removed: {error}", path.display());
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The size and free room of the filesystem of a path, as `df` says.
async fn room(machine: &Machine, path: &Path) -> Option<(u64, u64)> {
    let text = machine
        .output(
            "df",
            &["-B1", "--output=size,avail", &path.to_string_lossy()],
        )
        .await
        .ok()?;
    let mut numbers = text.lines().nth(1)?.split_whitespace();
    Some((numbers.next()?.parse().ok()?, numbers.next()?.parse().ok()?))
}

/// The disk of the system by category (only what sits on it), and the internal data disks of the
/// registry.
async fn overview(machine: &Machine) -> Overview {
    let system = machine.at("/");
    let device = fs::metadata(&system).map(|metadata| metadata.dev()).ok();
    let on_system = |path: &Path| {
        fs::symlink_metadata(path).is_ok_and(|metadata| Some(metadata.dev()) == device)
    };
    let sized = |paths: &[&str]| {
        paths
            .iter()
            .map(|path| machine.at(path))
            .filter(|path| on_system(path))
            .map(|path| size_of(&path))
            .sum::<u64>()
    };
    let (total, free) = room(machine, &system).await.unwrap_or_default();
    let categories = [
        ("packages", sized(&[ARCHIVES])),
        ("logs", sized(&[LOGS])),
        ("temporary", sized(&TEMPORARY)),
        ("apps", sized(&[APPS])),
    ]
    .into_iter()
    .map(|(id, bytes)| Category {
        id: id.to_owned(),
        bytes,
    })
    .collect();
    let registry = fs::read_to_string(machine.at(REGISTRY)).unwrap_or_default();
    let mut disks = Vec::new();
    for line in registry.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        // Only the disks of the NAS: network shares and external disks are none of its business.
        if fields.len() < 3 || fields.get(3).is_some_and(|kind| *kind != "device") {
            continue;
        }
        if let Some((total, free)) = room(machine, &machine.at(fields[2])).await {
            disks.push(Disk {
                name: fields[1].to_owned(),
                total,
                free,
            });
        }
    }
    Overview {
        total,
        free,
        categories,
        disks,
    }
}

/// Answers a request: an overview, a preview, or a cleanup with what it freed.
pub async fn handle(machine: &Machine, request: &Request) -> Result<serde_json::Value, String> {
    request.validate()?;
    let value = match (request.action, request.item) {
        (Action::Overview, _) => serde_json::to_value(overview(machine).await),
        (Action::Preview, Some(item)) => {
            serde_json::to_value(plan(machine, item, request.options).await?.outcome(item))
        }
        (Action::Clean, Some(item)) => {
            let before = plan(machine, item, request.options).await?;
            remove(machine, item, &before, request.options).await?;
            let after = plan(machine, item, request.options)
                .await
                .unwrap_or_default();
            let left: HashSet<&String> = after.entries.iter().collect();
            let gone: Vec<String> = before
                .entries
                .iter()
                .filter(|entry| !left.contains(entry))
                .cloned()
                .collect();
            let freed = Plan {
                bytes: before.bytes.saturating_sub(after.bytes),
                entries: if item == Item::Journal {
                    Vec::new()
                } else {
                    gone
                },
                ..Plan::default()
            };
            serde_json::to_value(freed.outcome(item))
        }
        _ => return Err("a preview or a cleanup names its item".into()),
    };
    value.map_err(|error| error.to_string())
}

/// The job: reads the request, answers it beside the database of the launcher, which reads it.
pub async fn run() -> Result<(), String> {
    run_on(&Machine::native()).await
}

pub async fn run_on(machine: &Machine) -> Result<(), String> {
    let request = fs::read(machine.at(REQUEST))
        .map_err(|error| format!("no cleanup request: {error}"))
        .and_then(|bytes| {
            serde_json::from_slice::<Request>(&bytes).map_err(|error| error.to_string())
        });
    let answer = match request {
        Ok(request) => match handle(machine, &request).await {
            Ok(value) => Answer {
                value: Some(value),
                error: None,
            },
            Err(error) => Answer {
                value: None,
                error: Some(error),
            },
        },
        Err(error) => Answer {
            value: None,
            error: Some(error),
        },
    };
    let result = machine.at(RESULT);
    let bytes = serde_json::to_vec(&answer).map_err(|error| error.to_string())?;
    fs::write(&result, bytes).map_err(|error| error.to_string())?;
    // Root writes it; the launcher reads it and removes it from its own folder. A refusal is in
    // the answer: the job itself went well.
    fs::set_permissions(&result, fs::Permissions::from_mode(0o644))
        .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "../../../../tests/services/cleanup/executor.rs"]
mod tests;
