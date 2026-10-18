use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

struct Fake {
    root: PathBuf,
    bin: PathBuf,
}

impl Fake {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "nas-cleanup-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&base);
        let (root, bin) = (base.join("root"), base.join("bin"));
        for folder in [
            "var/cache/apt/archives/partial",
            "var/log/journal",
            "tmp",
            "var/tmp",
            "var/lib/nas-os/backups",
            "var/lib/nas-os/apps",
            "etc/nas-os",
        ] {
            fs::create_dir_all(root.join(folder)).expect("folder");
        }
        fs::create_dir_all(&bin).expect("bin");
        let fake = Self { root, bin };
        let calls = fake.calls();
        let archives = fake.root.join("var/cache/apt/archives");
        fake.script(
            "apt-get",
            &format!(
                r#"echo "apt-get $*" >> {calls}
case "$*" in
  "-s autoremove") printf 'Reading package lists...\nRemv libold [1.0]\nRemv libgone [2.0]\n' ;;
  "-s autoclean") printf 'Del tool 1:2.0 [10 kB]\n' ;;
  clean) rm -f {archives}/*.deb {archives}/partial/* ;;
  autoclean) rm -f {archives}/tool_* ;;
esac"#,
                calls = calls.display(),
                archives = archives.display()
            ),
        );
        fake.script(
            "dpkg-query",
            r#"case "$*" in
  *Status*Installed-Size*) printf 'ii linux-image-6.12.0-amd64 6.12.0-1 100\nii linux-image-6.12.1-amd64 6.12.1-1 100\nii linux-image-6.12.2-amd64 6.12.2-1 100\nii linux-image-6.12.3-amd64 6.12.3-1 100\nii linux-image-amd64 6.12.3-1 1\nrc linux-image-6.11.0-amd64 6.11.0-1 100\n' ;;
  *Installed-Size*) shift 2; for package in "$@"; do echo 100; done ;;
  *) printf 'ii bash\nrc oldconfig\nrc otherconfig\n' ;;
esac"#,
        );
        fake.script(
            "dpkg",
            &format!(
                r#"if [ "$1" = --compare-versions ]; then
  [ "$2" != "$4" ] && [ "$(printf '%s\n%s\n' "$2" "$4" | sort -V | tail -n 1)" = "$2" ]
  exit
fi
echo "dpkg $*" >> {}"#,
                calls.display()
            ),
        );
        fake.script("uname", "echo 6.12.2-amd64");
        fake.script(
            "journalctl",
            &format!(
                r#"echo "journalctl $*" >> {}
echo 'Archived and active journals take up 1.5G in the file system.'"#,
                calls.display()
            ),
        );
        fake.script("df", "printf '     1B-blocks      Avail\\n1000 400\\n'");
        fake.script("systemctl", "exit 3");
        fake
    }

    fn calls(&self) -> PathBuf {
        self.bin.join("calls")
    }

    fn called(&self) -> String {
        fs::read_to_string(self.calls()).unwrap_or_default()
    }

    fn script(&self, name: &str, body: &str) {
        let path = self.bin.join(name);
        let draft = self.bin.join(format!(".{name}.draft"));
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

    fn file(&self, path: &str, bytes: usize) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        fs::write(path, vec![b'x'; bytes]).expect("file");
    }

    fn machine(&self, days: u32) -> Machine {
        Machine::new(self.root.clone(), Some(self.bin.clone()))
            .at_time(SystemTime::now() + DAY * days)
    }
}

fn ask(action: Action, item: Option<Item>) -> Request {
    Request {
        action,
        item,
        options: Options::default(),
    }
}

async fn outcome(machine: &Machine, action: Action, item: Item) -> Outcome {
    serde_json::from_value(
        handle(machine, &ask(action, Some(item)))
            .await
            .expect("answer"),
    )
    .expect("outcome")
}

#[test]
fn reads_what_the_programs_of_the_system_write() {
    assert_eq!(parse_size("1.5G"), Some(1_610_612_736));
    assert_eq!(parse_size("48K"), Some(49_152));
    assert_eq!(parse_size("12"), Some(12));
    assert_eq!(parse_size("2Q"), None);
    assert_eq!(
        journal_usage("Archived and active journals take up 512.0M in the file system."),
        Some(536_870_912)
    );
    assert_eq!(journal_usage("No journal files were found."), None);
    assert_eq!(
        simulated("Remv a [1.0]\nInst b\nRemv c\n", "Remv"),
        [
            ("a".to_owned(), "1.0".to_owned()),
            ("c".to_owned(), String::new())
        ]
    );
    for name in ["syslog.1", "dpkg.log.2.gz", "messages.old", "kern.log.3.xz"] {
        assert!(rotated(name), "{name}");
    }
    for name in ["syslog", "kern.log", "README", "Xorg.0.log"] {
        assert!(!rotated(name), "{name}");
    }
}

#[tokio::test]
async fn the_files_of_an_item_are_measured_then_removed_and_nothing_else() {
    let fake = Fake::new();
    fake.file("var/log/syslog", 10);
    fake.file("var/log/syslog.1", 20);
    fake.file("var/log/apt/history.log.2.gz", 30);
    fake.file("var/log/journal/system@1.journal~", 40);
    fake.file("tmp/old.txt", 50);
    fake.file("tmp/systemd-private-x/kept.txt", 60);
    fake.file("tmp/.X11-unix/X0", 1);
    fake.file("var/tmp/nested/old.bin", 70);
    std::os::unix::fs::symlink("/etc", fake.root.join("tmp/link")).expect("link");

    let soon = fake.machine(1);
    assert_eq!(
        outcome(&soon, Action::Preview, Item::Temporary).await.bytes,
        0
    );

    let later = fake.machine(30);
    let logs = outcome(&later, Action::Preview, Item::RotatedLogs).await;
    assert_eq!(logs.bytes, 50);
    assert_eq!(
        logs.entries,
        ["/var/log/apt/history.log.2.gz", "/var/log/syslog.1"]
    );
    let temporary = outcome(&later, Action::Preview, Item::Temporary).await;
    assert_eq!(
        temporary.entries,
        ["/tmp/link", "/tmp/old.txt", "/var/tmp/nested/old.bin"]
    );

    let freed = outcome(&later, Action::Clean, Item::Temporary).await;
    assert_eq!(freed.entries.len(), 3);
    assert!(fs::symlink_metadata(fake.root.join("tmp/link")).is_err());
    assert!(fake.root.join("tmp/systemd-private-x/kept.txt").exists());
    assert!(fake.root.join("tmp/.X11-unix/X0").exists());
    assert!(fs::metadata("/etc").is_ok_and(|metadata| metadata.is_dir()));

    let freed = outcome(&later, Action::Clean, Item::RotatedLogs).await;
    assert_eq!(freed.bytes, 50);
    assert!(fake.root.join("var/log/syslog").exists());
    assert!(fake.root.join("var/log/journal/system@1.journal~").exists());
}

#[tokio::test]
async fn the_nas_keeps_its_last_backup_and_what_an_update_may_still_use() {
    let fake = Fake::new();
    for name in ["20260101-010000", "20260201-010000", "20260301-010000"] {
        fake.file(&format!("var/lib/nas-os/backups/{name}.tar.gz"), 100);
    }
    fake.file("var/lib/nas-os/launcher-update.tar", 200);
    fake.file("var/lib/nas-os/apps/.nas-old-app.old/bin/app", 300);
    fake.file("var/lib/nas-os/apps/nas-old-app/bin/app", 300);
    let later = fake.machine(2);
    let backups = outcome(&later, Action::Preview, Item::NasBackups).await;
    assert_eq!(
        backups.entries,
        [
            "/var/lib/nas-os/backups/20260101-010000.tar.gz",
            "/var/lib/nas-os/backups/20260201-010000.tar.gz"
        ]
    );

    assert!(
        outcome(&fake.machine(0), Action::Preview, Item::NasLeftovers)
            .await
            .entries
            .is_empty()
    );
    let left = outcome(&later, Action::Preview, Item::NasLeftovers).await;
    assert_eq!(left.bytes, 500);

    fake.script("systemctl", "exit 0");
    let refused = handle(&later, &ask(Action::Clean, Some(Item::NasBackups))).await;
    assert_eq!(
        refused.expect_err("updating"),
        "an update of the NAS is under way"
    );
    fake.script("systemctl", "exit 3");
    outcome(&later, Action::Clean, Item::NasBackups).await;
    outcome(&later, Action::Clean, Item::NasLeftovers).await;
    assert!(fake
        .root
        .join("var/lib/nas-os/backups/20260301-010000.tar.gz")
        .exists());
    assert!(!fake
        .root
        .join("var/lib/nas-os/apps/.nas-old-app.old")
        .exists());
    assert!(fake
        .root
        .join("var/lib/nas-os/apps/nas-old-app/bin/app")
        .exists());
}

#[tokio::test]
async fn apt_and_the_journal_go_through_their_own_commands() {
    let fake = Fake::new();
    fake.file("var/cache/apt/archives/tool_1%3a2.0_amd64.deb", 100);
    fake.file("var/cache/apt/archives/bash_5.2_amd64.deb", 200);
    fake.file("var/cache/apt/archives/partial/half.deb", 50);
    fake.file("var/cache/apt/archives/lock", 0);
    let machine = fake.machine(0);

    let obsolete = outcome(&machine, Action::Preview, Item::AptObsolete).await;
    assert_eq!(obsolete.bytes, 100);
    let freed = outcome(&machine, Action::Clean, Item::AptObsolete).await;
    assert_eq!(freed.bytes, 100);
    assert_eq!(
        outcome(&machine, Action::Preview, Item::AptArchives)
            .await
            .bytes,
        250
    );
    assert_eq!(
        outcome(&machine, Action::Clean, Item::AptArchives)
            .await
            .bytes,
        250
    );
    assert!(fake.root.join("var/cache/apt/archives/lock").exists());

    let orphans = outcome(&machine, Action::Preview, Item::AptOrphans).await;
    assert_eq!(orphans.entries, ["libold", "libgone"]);
    assert_eq!(orphans.bytes, 200 * 1024);
    let configs = outcome(&machine, Action::Preview, Item::AptConfigs).await;
    assert_eq!(configs.entries, ["oldconfig", "otherconfig"]);
    let kernels = outcome(&machine, Action::Preview, Item::OldKernels).await;
    assert_eq!(kernels.entries, ["linux-image-6.12.0-amd64"]);
    let journal = outcome(&machine, Action::Preview, Item::Journal).await;
    assert_eq!(journal.bytes, 1_610_612_736 - 500 * 1024 * 1024);

    outcome(&machine, Action::Clean, Item::AptOrphans).await;
    outcome(&machine, Action::Clean, Item::AptConfigs).await;
    outcome(&machine, Action::Clean, Item::OldKernels).await;
    outcome(&machine, Action::Clean, Item::Journal).await;
    let called = fake.called();
    for call in [
        "apt-get -y autoremove",
        "dpkg --purge oldconfig otherconfig",
        "apt-get -y purge linux-image-6.12.0-amd64",
        "journalctl --vacuum-size=500M",
    ] {
        assert!(called.contains(call), "{call}: {called}");
    }

    fake.script("uname", "echo 6.99.0-custom");
    assert!(outcome(&machine, Action::Preview, Item::OldKernels)
        .await
        .entries
        .is_empty());

    fake.script("journalctl", "echo broken >&2; exit 1");
    let failed = handle(&machine, &ask(Action::Preview, Some(Item::Journal))).await;
    assert!(failed.expect_err("failed").contains("broken"));
}

#[tokio::test]
async fn the_overview_counts_what_sits_on_the_system_disk_and_the_data_disks() {
    let fake = Fake::new();
    fake.file("var/log/syslog", 10);
    fake.file("var/cache/apt/archives/a.deb", 20);
    fake.file("var/lib/nas-os/apps/app/bin", 30);
    fs::write(
        fake.root.join("etc/nas-os/storage-volumes.tsv"),
        "v1\tDati\t/srv/nas-volumes/v1\tdevice\nv2\tRete\t/mnt/share\tnetwork\n\
         v3\tChiavetta\t/srv/nas-volumes/v3\texternal\n",
    )
    .expect("registry");
    let overview: Overview = serde_json::from_value(
        handle(&fake.machine(0), &ask(Action::Overview, None))
            .await
            .expect("overview"),
    )
    .expect("overview");
    assert_eq!((overview.total, overview.free), (1000, 400));
    let bytes = |id: &str| {
        overview
            .categories
            .iter()
            .find(|category| category.id == id)
            .map(|category| category.bytes)
    };
    assert_eq!(bytes("logs"), Some(10));
    assert_eq!(bytes("packages"), Some(20));
    assert_eq!(bytes("apps"), Some(30));
    assert_eq!(
        overview.disks,
        [Disk {
            name: "Dati".to_owned(),
            total: 1000,
            free: 400
        }]
    );
}

#[tokio::test]
async fn the_job_answers_its_request_beside_the_database_of_the_launcher() {
    let fake = Fake::new();
    let machine = fake.machine(0);
    fs::write(
        machine.at(REQUEST),
        r#"{"action":"preview","item":"rotated-logs","options":{"temporaryDays":7,"journalMb":500}}"#,
    )
    .expect("request");
    run_on(&machine).await.expect("job");
    let answer: Answer =
        serde_json::from_slice(&fs::read(machine.at(RESULT)).expect("result")).expect("answer");
    assert_eq!(answer.value.expect("value")["item"], "rotated-logs");
    assert_eq!(
        fs::metadata(machine.at(RESULT))
            .expect("result")
            .permissions()
            .mode()
            & 0o777,
        0o644
    );

    fs::write(machine.at(REQUEST), r#"{"action":"clean"}"#).expect("request");
    run_on(&machine).await.expect("job");
    let answer: Answer =
        serde_json::from_slice(&fs::read(machine.at(RESULT)).expect("result")).expect("answer");
    assert_eq!(
        answer.error.as_deref(),
        Some("a preview or a cleanup names its item")
    );
    fs::write(machine.at(REQUEST), "not json").expect("request");
    run_on(&machine).await.expect("job");
    let answer: Answer =
        serde_json::from_slice(&fs::read(machine.at(RESULT)).expect("result")).expect("answer");
    assert!(answer.error.is_some());
}
