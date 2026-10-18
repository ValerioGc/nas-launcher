use super::*;

#[test]
fn anchors_file_paths_and_refuses_symlink_parents_and_virtual_filesystems() {
    let directory = std::env::temp_dir().join(format!("nas-antivirus-{}", now_ms()));
    fs::create_dir_all(directory.join("nested")).expect("test directory");
    let file = directory.join("nested/file");
    fs::write(&file, "sample").expect("test file");
    let (_parent, anchored) = parent(file.to_str().expect("path")).expect("anchored path");
    assert_eq!(
        fs::read_to_string(anchored).expect("read anchored file"),
        "sample"
    );
    std::os::unix::fs::symlink("nested", directory.join("link")).expect("test symlink");
    assert!(parent(directory.join("link/file").to_str().expect("path")).is_err());
    for path in [
        "/proc/test",
        "/sys/test",
        "/dev/test",
        "/var/lib/nas-os-antivirus/a",
        "/tmp/../etc/passwd",
    ] {
        assert!(parent(path).is_err());
    }
    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn reads_threats_with_colons_in_file_names_and_ignores_missing_files() {
    let path = std::env::temp_dir().join(format!("nas-antivirus-{}: sample", now_ms()));
    fs::write(&path, "sample").expect("test file");
    let found = detection(
        &format!("{}: Eicar-Test-Signature FOUND", path.display()),
        10,
        2,
    )
    .expect("detection");
    assert_eq!(found.path, path.to_str().expect("path"));
    assert_eq!(found.threat, "Eicar-Test-Signature");
    assert_eq!(found.id, "10-2");
    assert!(detection("/a/file/that/does/not/exist: Threat FOUND", 10, 2).is_none());
    assert!(detection(&format!("{}: OK", path.display()), 10, 2).is_none());
    fs::remove_file(path).expect("cleanup");
}

thread_local! {
    static ROOT: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}
pub(super) fn path(value: &str) -> Option<PathBuf> {
    ROOT.with(|root| {
        root.borrow().as_ref().map(|root| {
            root.join(if value == STATE {
                "private"
            } else {
                value.rsplit('/').next().expect("filename")
            })
        })
    })
}
pub(super) fn program(value: &str) -> Option<PathBuf> {
    ROOT.with(|root| {
        root.borrow()
            .as_ref()
            .map(|root| root.join(Path::new(value).file_name().expect("program")))
    })
}
struct Harness {
    root: PathBuf,
}
impl Harness {
    fn new() -> Self {
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "nas-av-native-{}-{}-{}",
            std::process::id(),
            now_ms(),
            SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("private")).expect("private state");
        ROOT.with(|value| *value.borrow_mut() = Some(root.clone()));
        let harness = Self { root };
        harness.script("id", "/usr/bin/id -u");
        harness.script("systemctl", "echo stopping");
        harness.script("freshclam", "exit 0");
        harness.script("clamscan", "exit 0");
        harness.script("nas-os-axum", "exit 0");
        harness
    }

    fn script(&self, name: &str, body: &str) {
        let path = self.root.join(name);
        let draft = self.root.join(format!(".{name}.draft"));
        fs::write(&draft, format!("#!/bin/sh\n{body}\n")).expect("stub draft");
        let _ = fs::remove_file(&path);
        let copied = std::process::Command::new("cp")
            .arg(&draft)
            .arg(&path)
            .status()
            .expect("cp");
        assert!(copied.success(), "stub program");
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("executable stub");
    }
    fn sample(&self) -> PathBuf {
        let path = self.root.join("sample");
        fs::write(&path, "infected test fixture").expect("sample");
        path
    }
    async fn run(&self, job: Job) -> Result<(), String> {
        fs::write(
            native_path(REQUEST),
            serde_json::to_vec(&job).expect("request"),
        )
        .expect("request file");
        super::run().await
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        ROOT.with(|value| *value.borrow_mut() = None);
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn file_job(path: &Path, auto_quarantine: bool) -> Job {
    Job::Scan {
        request: ScanRequest {
            kind: ScanKind::File,
            paths: vec![path.to_str().expect("path").into()],
            skip_unchanged: false,
        },
        options: ScanOptions::default(),
        auto_quarantine,
    }
}

#[tokio::test]
async fn scans_quarantines_restores_and_deletes_using_native_process_contracts() {
    let harness = Harness::new();
    let sample = harness.sample();
    harness.script(
        "clamscan",
        &format!(
            "printf '%s\\n' '{}: Eicar-Test-Signature FOUND' 'Scanned files: 1'; exit 1",
            sample.display()
        ),
    );
    harness.run(file_job(&sample, false)).await.expect("scan");
    let mut report = load_report().await.expect("report");
    assert_eq!((report.scanned, report.found), (1, 1));
    let id = report.detections[0].id.clone();
    harness
        .run(Job::Quarantine { id: id.clone() })
        .await
        .expect("quarantine");
    assert!(!sample.exists());
    report = load_report().await.expect("report");
    assert!(report.detections.is_empty());
    assert_eq!(report.quarantine.len(), 1);
    fs::write(&sample, "new file").expect("existing restore target");
    assert!(harness.run(Job::Restore { id: id.clone() }).await.is_err());
    assert_eq!(fs::read_to_string(&sample).expect("contents"), "new file");
    fs::remove_file(&sample).expect("remove conflict");
    harness.run(Job::Restore { id }).await.expect("restore");
    assert_eq!(
        fs::read_to_string(&sample).expect("contents"),
        "infected test fixture"
    );
    harness
        .run(file_job(&sample, true))
        .await
        .expect("automatic quarantine");
    let report = load_report().await.expect("report");
    harness
        .run(Job::Delete {
            id: report.quarantine[0].detection.id.clone(),
        })
        .await
        .expect("delete");
    assert!(load_report().await.expect("report").quarantine.is_empty());
}

#[tokio::test]
async fn rejects_changed_files_forged_findings_partial_scans_and_failed_programs() {
    let harness = Harness::new();
    let sample = harness.sample();
    let mut report = Report {
        detections: vec![
            detection(&format!("{}: Threat FOUND", sample.display()), 1, 0).expect("detection"),
        ],
        ..Report::default()
    };
    harness.script("clamscan", "exit 0");
    assert!(quarantine("1-0", &mut report)
        .await
        .expect_err("clean recheck")
        .contains("no longer"));
    report.detections[0].inode += 1;
    assert!(quarantine("1-0", &mut report)
        .await
        .expect_err("changed inode")
        .contains("changed"));
    for job in [
        Job::Quarantine {
            id: "../etc".into(),
        },
        Job::Restore {
            id: "../etc".into(),
        },
        Job::Delete {
            id: "../etc".into(),
        },
        Job::Quarantine {
            id: "missing".into(),
        },
        Job::Restore {
            id: "missing".into(),
        },
        Job::Delete {
            id: "missing".into(),
        },
    ] {
        assert!(harness.run(job).await.is_err());
    }
    harness.script("clamscan", "echo 'Unreadable file ERROR'; exit 0");
    assert!(harness
        .run(file_job(&sample, false))
        .await
        .expect_err("partial scan")
        .contains("incomplete"));
    harness.script("clamscan", "echo 'bad database' >&2; exit 2");
    assert!(harness
        .run(file_job(&sample, false))
        .await
        .expect_err("native failure")
        .contains("bad database"));
    assert!(load_report()
        .await
        .expect("persisted failure")
        .error
        .is_some());
    harness.script("clamscan", "exit 1");
    assert!(harness
        .run(file_job(&sample, false))
        .await
        .expect_err("unreadable findings")
        .contains("could not be read"));
    harness.script("freshclam", "echo 'update failed' >&2; exit 1");
    assert!(harness
        .run(Job::Enable)
        .await
        .expect_err("updater failure")
        .contains("update failed"));
    fs::remove_file(harness.root.join("freshclam")).expect("missing program");
    assert!(command("freshclam", &[]).await.is_err());
}

#[tokio::test]
async fn validates_scopes_options_requests_and_persists_actual_counts() {
    let harness = Harness::new();
    let sample = harness.sample();
    let excluded_sample = harness.root.join("excluded");
    fs::write(&excluded_sample, "sample").expect("file");
    harness.script("clamscan", &format!("printf '%s\\n' '{}: Threat FOUND' '{}: Threat FOUND' '{}: OK' 'Scanned files: invalid' 'Scanned files: 3'; exit 0", sample.display(), excluded_sample.display(), sample.display()));
    let options = ScanOptions {
        paths: vec![harness.root.to_str().expect("path").into()],
        exclusions: vec![
            excluded_sample.to_str().expect("path").into(),
            "/tmp/[a].*+?(){{}}^$|\\".into(),
        ],
        ..ScanOptions::default()
    };
    harness
        .run(Job::Scan {
            request: ScanRequest {
                kind: ScanKind::Quick,
                paths: vec![],
                skip_unchanged: false,
            },
            options,
            auto_quarantine: false,
        })
        .await
        .expect("quick scope");
    assert_eq!(load_report().await.expect("report").found, 1);
    assert_eq!(load_report().await.expect("report").scanned, 3);
    assert!(harness.run(file_job(&harness.root, false)).await.is_err());
    assert!(harness
        .run(file_job(Path::new("/proc/version"), false))
        .await
        .is_err());
    assert!(harness
        .run(file_job(Path::new("/missing-native-file"), false))
        .await
        .is_err());
    harness.script("clamscan", "exit 0");
    for kind in [ScanKind::Full, ScanKind::Quick, ScanKind::Custom] {
        harness
            .run(Job::Scan {
                request: ScanRequest {
                    kind,
                    paths: vec![sample.to_str().expect("path").into()],
                    skip_unchanged: false,
                },

                options: ScanOptions {
                    paths: vec![harness.root.to_str().expect("path").into()],
                    ..ScanOptions::default()
                },
                auto_quarantine: false,
            })
            .await
            .expect("scan mode");
    }
    fs::write(native_path(REQUEST), vec![b' '; 65537]).expect("oversize request");
    assert!(super::run()
        .await
        .expect_err("bounded input")
        .contains("too large"));
    fs::write(native_path(REQUEST), "invalid").expect("malformed input");
    assert!(super::run().await.is_err());
}

fn folder_job(folder: &Path, kind: ScanKind, skip_unchanged: bool) -> Job {
    Job::Scan {
        request: ScanRequest {
            kind,
            paths: vec![folder.to_str().expect("path").into()],
            skip_unchanged,
        },
        options: ScanOptions {
            paths: vec![folder.to_str().expect("path").into()],
            exclusions: vec![folder.join("skip").to_str().expect("path").into()],
            ..ScanOptions::default()
        },
        auto_quarantine: false,
    }
}

#[tokio::test]
async fn a_quick_scan_sends_to_clamav_only_what_changed_since_it_was_found_clean() {
    let harness = Harness::new();
    let folder = harness.root.join("data");
    fs::create_dir_all(folder.join("inner")).expect("folders");
    fs::create_dir_all(folder.join("skip")).expect("excluded folder");
    fs::write(folder.join("a.txt"), "a").expect("a");
    fs::write(folder.join("inner/b.txt"), "b").expect("b");
    fs::write(folder.join("skip/c.txt"), "c").expect("excluded");
    std::os::unix::fs::symlink("/etc/hostname", folder.join("link")).expect("link");
    let calls = harness.root.join("calls");

    harness.script(
        "clamscan",
        &format!(
            r#"list=''
for arg in "$@"; do case $arg in --file-list=*) list=${{arg#--file-list=}} ;; esac; done
[ -n "$list" ] || {{ echo "whole $*" >> {calls}; exit 0; }}
cat "$list" >> {calls}
found=0
while IFS= read -r file; do
  case $file in *infected*) printf '%s: Threat FOUND
' "$file"; found=1 ;; *) printf '%s: OK
' "$file" ;; esac
done < "$list"
exit $found"#,
            calls = calls.display()
        ),
    );
    let listed = || fs::read_to_string(&calls).unwrap_or_default();

    harness
        .run(folder_job(&folder, ScanKind::Quick, false))
        .await
        .expect("first quick scan");
    let report = load_report().await.expect("report");
    assert_eq!((report.planned, report.skipped, report.scanned), (2, 0, 2));
    assert!(listed().contains("a.txt") && listed().contains("inner/b.txt"));
    assert!(!listed().contains("c.txt") && !listed().contains("link"));

    fs::remove_file(&calls).expect("reset");
    harness
        .run(folder_job(&folder, ScanKind::Quick, false))
        .await
        .expect("second quick scan");
    let report = load_report().await.expect("report");
    assert_eq!((report.planned, report.skipped), (0, 2));
    assert!(listed().is_empty());

    fs::write(folder.join("a.txt"), "changed").expect("changed");
    fs::write(folder.join("infected.txt"), "x").expect("new");
    harness
        .run(folder_job(&folder, ScanKind::Custom, true))
        .await
        .expect("custom scan leaving out the unchanged");
    let report = load_report().await.expect("report");
    assert_eq!((report.planned, report.skipped, report.found), (2, 1, 1));

    fs::remove_file(&calls).expect("reset");
    harness
        .run(folder_job(&folder, ScanKind::Quick, false))
        .await
        .expect("quick after a finding");
    assert_eq!(load_report().await.expect("report").planned, 1);
    assert!(listed().contains("infected.txt"));

    harness
        .run(folder_job(&folder, ScanKind::Custom, false))
        .await
        .expect("whole custom scan");
    assert!(listed().contains("whole"));
    let kept = load_report().await.expect("report");
    assert_eq!(kept.cache_files, 2);
    assert!(kept.cache_bytes > 0);
    harness.run(Job::Forget).await.expect("forget");
    let forgotten = load_report().await.expect("report");
    assert_eq!((forgotten.cache_files, forgotten.cache_bytes), (0, 0));
    harness.run(Job::Forget).await.expect("forget twice");
    harness
        .run(folder_job(&folder, ScanKind::Quick, false))
        .await
        .expect("quick after forgetting");
    assert_eq!(load_report().await.expect("report").planned, 3);
}

#[tokio::test]
async fn configures_lifecycle_scans_once_per_boot_and_only_on_actual_shutdown() {
    let harness = Harness::new();
    lifecycle(true).await.expect("missing settings means off");
    let sample = harness.sample();
    let selected = LifecycleScan {
        enabled: true,
        kind: ScanKind::File,
        paths: vec![sample.to_str().expect("path").into()],
    };
    let settings = Settings {
        enabled: true,
        boot_scan: selected.clone(),
        shutdown_scan: selected,
        ..Settings::default()
    };
    harness.run(Job::Enable).await.expect("enable updater");
    harness
        .run(Job::Resume { enabled: true })
        .await
        .expect("resume updater");
    harness
        .run(Job::Configure {
            settings: Box::new(settings.clone()),
        })
        .await
        .expect("configure");
    lifecycle(true).await.expect("same boot skips scan");
    assert_eq!(load_report().await.expect("report").at, 0);
    fs::write(state_file("boot-id"), "previous-boot").expect("previous boot");
    lifecycle(true).await.expect("new boot scan");
    fs::write(state_file("boot-id"), "another-boot").expect("another boot");
    harness.script("clamscan", "exit 2");
    lifecycle(true)
        .await
        .expect("failed boot scan still arms shutdown");
    let report = load_report().await.expect("scan report");
    assert!(report.error.is_some());
    assert_eq!(report.kind, Some(ScanKind::File));
    assert!(report.at > 0);
    harness.script("systemctl", "echo running");
    lifecycle(false)
        .await
        .expect("launcher restart is not shutdown");
    assert_eq!(load_report().await.expect("report").at, report.at);
    harness.script("systemctl", "echo stopping");
    harness.script("clamscan", "exit 2");
    assert!(lifecycle(false).await.is_err());
    let notices: Vec<LifecycleNotice> = serde_json::from_slice(
        &fs::read(native_path("/var/lib/nas-os/antivirus.notices.json")).expect("notice file"),
    )
    .expect("notices");
    assert_eq!(notices.len(), 3);
    assert!(notices[1].error.is_some());
    let mut disabled = settings;
    disabled.enabled = false;
    harness
        .run(Job::Configure {
            settings: Box::new(disabled),
        })
        .await
        .expect("disable lifecycle");
    lifecycle(true).await.expect("disabled at boot");
    lifecycle(false).await.expect("disabled at shutdown");
    harness.run(Job::Disable).await.expect("disable updater");
    harness
        .run(Job::Resume { enabled: false })
        .await
        .expect("resume disabled");
}

#[test]
fn tells_why_a_command_failed_from_either_stream() {
    assert_eq!(
        failed("freshclam", b"ERROR: Can't download daily.cvd\n", b""),
        "freshclam failed: ERROR: Can't download daily.cvd"
    );
    assert_eq!(
        failed("systemctl", b"", b"Unit not found.\n"),
        "systemctl failed: Unit not found."
    );
    assert_eq!(
        failed("clamscan", b"  ", b""),
        "clamscan failed without a message"
    );
    let long = format!("{}END", "x".repeat(3000));
    let reason = failed("freshclam", long.as_bytes(), b"");
    assert!(reason.ends_with("END"));
    assert_eq!(reason.chars().count(), "freshclam failed: ".len() + 2000);
}
