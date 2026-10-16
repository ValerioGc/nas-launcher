//! A machine of its own for the launcher: what it runs as root is a fake on `PATH` that writes down
//! how it was called, the forges are one local server that answers like GitHub and GitLab, and the
//! database, the applications, the logs and the catalogue live in one temporary directory. Every
//! switch is real: nothing reaches the real machine.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use std::{
    collections::HashMap,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

/// A release on the forge: the files of its package, and how its checksum is published.
#[derive(Clone)]
pub struct Release {
    pub version: &'static str,
    pub asset: String,
    pub files: Vec<(&'static str, String)>,
    pub checksum: Checksum,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Checksum {
    Right,
    Wrong,
    Missing,
}

#[derive(Clone, Default)]
struct Forge {
    base: String,
    releases: Arc<Mutex<HashMap<String, Release>>>,
    assets: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

pub struct Machine {
    pub root: PathBuf,
    forge: Forge,
}

static MACHINE: OnceLock<Machine> = OnceLock::new();
// Reset and power tests share the simulation of the running root operations.
pub static ROOT_OPERATIONS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The applications of the catalogue, each for one test, released from `tests/<id>`.
pub const CATALOGUE: &[(&str, &str)] = &[
    ("probe", "github"),
    ("labprobe", "gitlab"),
    ("badsum", "github"),
    ("nosum", "github"),
    ("noasset", "github"),
    ("greedy", "github"),
    ("norelease", "github"),
    ("checked", "github"),
    ("started", "github"),
    ("iconic", "github"),
    ("worker", "github"),
];

/// The fakes of what the launcher runs: each writes its name and arguments to `calls`, and answers
/// with the file `answers/<name>` when there is one, failing when `fail/<name>` exists, or
/// `fail/<name>@<last argument>` for that call alone; `answers/<name>@<first argument>` answers that
/// subcommand alone. A unit is
/// active for `systemctl is-active` while `active/<unit>` exists.
const FAKE: &str = r#"#!/usr/bin/env bash
name=$(basename "$0")
printf '%s %s\n' "$name" "$*" >>"$NAS_MACHINE/calls"
if [[ $name == systemctl && $1 == is-active ]]; then
  status=0
  for unit in "${@:2}"; do
    [[ $unit == --* ]] && continue
    if [[ -f $NAS_MACHINE/active/$unit ]]; then [[ $* == *--quiet* ]] || echo active
    else [[ $* == *--quiet* ]] || echo inactive; status=3; fi
  done
  exit $status
fi
[[ -f $NAS_MACHINE/fail/$name ]] && { cat "$NAS_MACHINE/fail/$name" >&2; exit 1; }
[[ -f "$NAS_MACHINE/fail/$name@${*: -1}" ]] && { cat "$NAS_MACHINE/fail/$name@${*: -1}" >&2; exit 1; }
[[ -f "$NAS_MACHINE/answers/$name@$1" ]] && { cat "$NAS_MACHINE/answers/$name@$1"; exit 0; }
[[ -f $NAS_MACHINE/answers/$name ]] && cat "$NAS_MACHINE/answers/$name"
exit 0
"#;

const FAKES: &[&str] = &[
    "systemctl",
    "apt",
    "smartctl",
    "lsblk",
    "nmcli",
    "clamscan",
    "loginctl",
    "wipefs",
    "parted",
    "mkfs.ext4",
    "udevadm",
    "blkid",
    "mount",
    "umount",
    "partprobe",
    "e2label",
    "findmnt",
    "cryptsetup",
    "systemd-creds",
    "chown",
    "nft",
];

fn tarball(directory: &std::path::Path, files: &[(&'static str, String)]) -> Vec<u8> {
    let content = directory.join("content");
    let _ = std::fs::remove_dir_all(&content);
    for (path, text) in files {
        let file = content.join(path);
        std::fs::create_dir_all(file.parent().expect("parent")).expect("package folder");
        std::fs::write(&file, text).expect("package file");
    }
    let archive = directory.join("package.tar.gz");
    let status = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&content)
        .arg(".")
        .status()
        .expect("tar");
    assert!(status.success());
    std::fs::read(archive).expect("archive")
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

async fn github(
    State(forge): State<Forge>,
    Path((owner, name)): Path<(String, String)>,
) -> Response {
    release(&forge, &format!("{owner}/{name}"), "assets")
}

async fn gitlab(State(forge): State<Forge>, Path(project): Path<String>) -> Response {
    release(&forge, &project.replace("%2F", "/"), "links")
}

fn release(forge: &Forge, repository: &str, field: &str) -> Response {
    let Some(release) = forge
        .releases
        .lock()
        .expect("lock")
        .get(repository)
        .cloned()
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut assets = vec![serde_json::json!({
        "name": release.asset,
        "url": format!("{}/assets/{}", forge.base, release.asset),
    })];
    if release.checksum != Checksum::Missing {
        assets.push(serde_json::json!({
            "name": format!("{}.sha256", release.asset),
            "url": format!("{}/assets/{}.sha256", forge.base, release.asset),
        }));
    }
    let assets = if field == "links" {
        serde_json::json!({ "links": assets })
    } else {
        serde_json::Value::Array(assets)
    };
    Json(serde_json::json!({ "tag_name": format!("v{}", release.version), "assets": assets }))
        .into_response()
}

/// The Email application takes every event, but refuses a code for an address that says so.
async fn mail_event(body: String) -> Response {
    if body.contains("refused@") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "no such address"})),
        )
            .into_response();
    }
    if body.contains("broken@") {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    StatusCode::OK.into_response()
}

async fn asset(State(forge): State<Forge>, Path(file): Path<String>) -> Response {
    match forge.assets.lock().expect("lock").get(&file) {
        Some(bytes) => bytes.clone().into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

impl Machine {
    /// The machine of this test binary, made the first time it is asked for.
    pub fn get() -> &'static Machine {
        MACHINE.get_or_init(|| {
            let root = std::env::temp_dir().join(format!("nas-os-machine-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let bin = root.join("bin");
            for folder in [
                "bin", "answers", "fail", "active", "apps", "logs", "state", "sockets", "dropins",
                "credstore", "runtime",
            ] {
                std::fs::create_dir_all(root.join(folder)).expect("machine folder");
            }
            for name in FAKES {
                let path = bin.join(name);
                std::fs::write(&path, FAKE).expect("fake command");
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                    .expect("executable");
            }

            let mailer = root.join("apps/mailer");
            std::fs::create_dir_all(&mailer).expect("mail application");
            std::fs::write(
                mailer.join("nas-app.json"),
                serde_json::json!({
                    "manifest_version": 1,
                    "app": { "id": "mailer", "name": "Email" },
                    "provides": [{ "capability": "mail-events", "version": 1, "endpoint": "/internal/events" }],
                })
                .to_string(),
            )
            .expect("mail manifest");
            let mail_root = root.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            let forge = Forge::default();
            let served = forge.clone();
            std::thread::spawn(move || {
                let runtime = tokio::runtime::Runtime::new().expect("runtime");
                runtime.block_on(async move {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                        .await
                        .expect("forge");
                    let base = format!("http://{}", listener.local_addr().expect("address"));
                    let served = Forge { base: base.clone(), ..served };
                    sender.send(base).expect("base");
                    let router = Router::new()
                        .route("/repos/{owner}/{name}/releases/latest", get(github))
                        .route("/projects/{project}/releases/permalink/latest", get(gitlab))
                        .route("/assets/{file}", get(asset))
                        .with_state(served);
                    // The Email application, always answering on its socket.
                    let mail = Router::new()
                        .route("/api/status", get(|| async { Json(serde_json::json!({"sender": true})) }))
                        .route("/internal/events", axum::routing::post(mail_event));
                    let sockets = mail_root.join("sockets");
                    let mail_listener =
                        tokio::net::UnixListener::bind(sockets.join("mailer.sock")).expect("mail socket");
                    tokio::spawn(async move { axum::serve(mail_listener, mail).await.expect("mail") });
                    axum::serve(listener, router).await.expect("forge");
                });
            });
            let base = receiver.recv().expect("forge address");

            let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
            let state = root.join("state");
            unsafe {
                std::env::set_var("PATH", path);
                std::env::set_var("NAS_MACHINE", &root);
                std::env::set_var("NAS_DB_PATH", state.join("launcher.db"));
                std::env::set_var("NAS_APPS_DIR", root.join("apps"));
                std::env::set_var("NAS_LOG_DIR", root.join("logs"));
                std::env::set_var("NAS_CATALOGUE_PATH", root.join("catalogue.json"));
                std::env::set_var("NAS_GITHUB_API", &base);
                std::env::set_var("NAS_GITLAB_API", &base);
                std::env::set_var("NAS_PACKAGES_MODE", "real");
                std::env::set_var("NAS_APT_MODE", "real");
                for mode in ["APPS", "FIREWALL", "TOKEN", "PROCESSES", "NETWORK", "RESET", "STORAGE"] {
                    std::env::set_var(format!("NAS_{mode}_MODE"), "real");
                }
                std::env::set_var("NAS_APP_SOCKETS", root.join("sockets"));
                std::env::set_var("NAS_APP_DROPINS", root.join("dropins"));
                std::env::set_var("CREDENTIALS_DIRECTORY", root.join("credentials"));
                std::env::set_var("NAS_WIZARD_UNIT", root.join("nas-wizard.service"));
                std::env::set_var("NAS_WIZARD_HOME", root.join("wizard"));
                std::env::set_var("NAS_CONFIGURATION_STATE", root.join("configuration"));
                std::env::set_var("NAS_STORAGE_STATE_DIR", &state);
                // Encryption: the socket a test serves with the root helper, its keys and runtime.
                std::env::set_var("NAS_STORAGE_CRYPT_SOCKET", root.join("crypt.sock"));
                std::env::set_var("NAS_CREDSTORE", root.join("credstore"));
                std::env::set_var("NAS_CRYPTTAB", root.join("crypttab"));
                std::env::set_var("RUNTIME_DIRECTORY", root.join("runtime"));
                std::env::set_var("NAS_ENCRYPTION_MODE", "real");
                std::env::set_var("NAS_STORAGE_REGISTRY", root.join("storage-volumes.tsv"));
                std::env::set_var("NAS_NETWORK_DISKS", root.join("network-disks.json"));
                std::env::set_var("NAS_NETWORK_DISKS_MODE", "real");
                // No user of the wizard: the reconciliation publishes no volume.
                std::env::set_var("NAS_TERMINAL_DROPIN", root.join("terminal"));
                std::env::set_var("NAS_LAUNCHER_SOCKET", root.join("launcher.sock"));
                std::env::set_var("NAS_LAUNCHER_REPOSITORY", "tests/nas-launcher");
                std::env::set_var("NAS_GATEWAY_BINARY", root.join("versions/nas-os-gateway"));
                std::env::set_var("NAS_WIZARD_BINARY", root.join("versions/nas-wizard"));
            }
            // The gateway and the wizard on the machine, which say their version.
            std::fs::create_dir_all(root.join("versions")).expect("versions folder");
            for program in ["nas-os-gateway", "nas-wizard"] {
                let path = root.join("versions").join(program);
                std::fs::write(&path, "#!/bin/sh\necho 0.1.0\n").expect("version program");
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                    .expect("executable");
            }
            for relative in [
                "wizard/wizard/bin/nas-wizard",
                "wizard/packages/os_configuration/scripts/uninstall.sh",
            ] {
                let path = root.join(relative);
                std::fs::create_dir_all(path.parent().expect("parent")).expect("wizard folder");
                std::fs::write(&path, "#!/bin/sh\nexit 0\n").expect("wizard fixture");
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                    .expect("executable");
            }
            std::fs::create_dir_all(root.join("configuration")).expect("configuration folder");
            std::fs::write(root.join("configuration/original-ready"), "").expect("backup marker");
            let apps = CATALOGUE
                .iter()
                .map(|(id, provider)| {
                    serde_json::json!({
                        "id": id,
                        "name": id,
                        "info": "A test application.",
                        "provider": provider,
                        "repository": format!("tests/{id}"),
                        "asset": format!("{id}-{{version}}.tar.gz"),
                    })
                })
                .collect::<Vec<_>>();
            // What the launcher runs has to be the fake, never the command of this machine.
            for name in FAKES {
                let found = std::process::Command::new("sh")
                    .args(["-c", &format!("command -v {name}")])
                    .output()
                    .expect("command -v");
                let found = String::from_utf8_lossy(&found.stdout).trim().to_owned();
                assert_eq!(PathBuf::from(&found), bin.join(name), "{name} is not the fake");
            }
            std::fs::write(root.join("nas-wizard.service"), "[Unit]\n").expect("wizard unit");
            // The disks of the machine: the system on sda, an ext4 volume on sdy1, sdz blank, open
            // LUKS containers on sdw1 and sds1, a closed one on sdt1, a data disk to format on sdv.
            std::fs::write(
                root.join("answers/lsblk"),
                serde_json::json!({ "blockdevices": [
                    { "name": "sda", "path": "/dev/sda", "type": "disk", "ro": false, "children": [
                        { "name": "sda1", "path": "/dev/sda1", "type": "part", "fstype": "ext4",
                          "mountpoints": ["/"], "ro": false }
                    ]},
                    { "name": "sdy", "path": "/dev/sdy", "type": "disk", "ro": false, "children": [
                        { "name": "sdy1", "path": "/dev/sdy1", "type": "part", "fstype": "ext4",
                          "mountpoints": [null], "ro": false }
                    ]},
                    { "name": "sdz", "path": "/dev/sdz", "type": "disk", "ro": false },
                    { "name": "sdv", "path": "/dev/sdv", "type": "disk", "ro": false, "children": [
                        { "name": "sdv1", "path": "/dev/sdv1", "type": "part", "fstype": "ext4",
                          "uuid": "OLD-V", "mountpoints": [null], "ro": false }
                    ]},
                    { "name": "sdt", "path": "/dev/sdt", "type": "disk", "ro": false, "children": [
                        { "name": "sdt1", "path": "/dev/sdt1", "type": "part", "fstype": "crypto_LUKS",
                          "uuid": "LUKS-T", "ro": false }
                    ]},
                    { "name": "sds", "path": "/dev/sds", "type": "disk", "ro": false, "children": [
                        { "name": "sds1", "path": "/dev/sds1", "type": "part", "fstype": "crypto_LUKS",
                          "uuid": "LUKS-S", "ro": false, "children": [
                            { "name": "nas-luks-luks-s", "path": "/dev/mapper/nas-luks-luks-s",
                              "type": "crypt", "fstype": "ext4", "uuid": "INNER-S",
                              "mountpoints": [null], "ro": false }
                        ]}
                    ]},
                    { "name": "sdw", "path": "/dev/sdw", "type": "disk", "ro": false, "children": [
                        { "name": "sdw1", "path": "/dev/sdw1", "type": "part", "fstype": "crypto_LUKS",
                          "uuid": "LUKS-W", "ro": false, "children": [
                            { "name": "nas-luks-luks-w", "path": "/dev/mapper/nas-luks-luks-w",
                              "type": "crypt", "fstype": "ext4", "uuid": "INNER-W",
                              "mountpoints": [null], "ro": false }
                        ]}
                    ]},
                    { "name": "sdu", "path": "/dev/sdu", "type": "disk", "ro": false, "rm": true,
                      "tran": "usb", "children": [
                        { "name": "sdu1", "path": "/dev/sdu1", "type": "part", "fstype": "vfat",
                          "uuid": "ABCD-1234", "mountpoints": ["/media/usb"], "ro": false }
                    ]}
                ]})
                .to_string(),
            )
            .expect("lsblk answer");
            // NetworkManager with a Wi-Fi joined and a cable: every listing answers with these rows.
            std::fs::write(
                root.join("answers/nmcli"),
                "wlan9:wifi:connected:Home\neth9:ethernet:connected:Wired\n",
            )
            .expect("nmcli answer");
            // Programming the alarm fails on this machine, and nothing else does.
            std::fs::write(root.join("fail/systemctl@nas-os-wake.service"), "no alarm")
                .expect("wake failure");
            std::fs::write(
                root.join("answers/smartctl"),
                r#"{"smartctl":{"exit_status":0},"smart_status":{"passed":true}}"#,
            )
            .expect("smartctl answer");
            std::fs::write(
                root.join("answers/apt"),
                "Listing...\nbash/stable-security 5.2.37-2 amd64 [upgradable from: 5.2.15-2]\n\
                 curl/stable 8.14.1-2 amd64 [upgradable from: 8.14.1-1]\n",
            )
            .expect("apt answer");
            std::fs::write(
                root.join("catalogue.json"),
                serde_json::json!({ "version": 1, "apps": apps }).to_string(),
            )
            .expect("catalogue");
            Machine {
                root,
                forge: Forge { base, ..forge },
            }
        })
    }

    /// Publishes `release` as the latest of `repository`.
    pub fn publish(&self, repository: &str, release: Release) {
        let folder = self
            .root
            .join("packages")
            .join(repository.replace('/', "-"));
        std::fs::create_dir_all(&folder).expect("package folder");
        let bytes = tarball(&folder, &release.files);
        let declared = match release.checksum {
            Checksum::Wrong => "0".repeat(64),
            _ => sha256(&bytes),
        };
        let mut assets = self.forge.assets.lock().expect("lock");
        assets.insert(
            format!("{}.sha256", release.asset),
            format!("{declared}  {}\n", release.asset).into_bytes(),
        );
        assets.insert(release.asset.clone(), bytes);
        self.forge
            .releases
            .lock()
            .expect("lock")
            .insert(repository.to_owned(), release);
    }

    /// Makes `systemctl start <unit>` fail with `reason`, or succeed again with `None`. Only the test
    /// that uses `unit` may change it.
    pub fn fail_unit(&self, unit: &str, reason: Option<&str>) {
        let path = self.root.join("fail").join(format!("systemctl@{unit}"));
        match reason {
            Some(reason) => std::fs::write(path, reason).expect("failure"),
            None => {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    /// Makes `unit` active for `systemctl is-active`, or inactive again.
    pub fn activate(&self, unit: &str, active: bool) {
        let path = self.root.join("active").join(unit);
        if active {
            std::fs::write(path, "").expect("active unit");
        } else {
            let _ = std::fs::remove_file(path);
        }
    }

    /// The commands run so far whose line starts with `prefix`.
    pub fn calls(&self, prefix: &str) -> Vec<String> {
        std::fs::read_to_string(self.root.join("calls"))
            .unwrap_or_default()
            .lines()
            .filter(|line| line.starts_with(prefix))
            .map(str::to_owned)
            .collect()
    }
}

/// A package with a manifest for `id`, and `extra` files beside it.
pub fn package(id: &str, version: &'static str, extra: &[(&'static str, &str)]) -> Release {
    let mut files = vec![(
        "nas-app.json",
        format!(r#"{{"manifest_version":1,"app":{{"id":"{id}","name":"{id}"}}}}"#),
    )];
    files.extend(extra.iter().map(|(path, text)| (*path, (*text).to_owned())));
    Release {
        version,
        asset: format!("{id}-{version}.tar.gz"),
        files,
        checksum: Checksum::Right,
    }
}
