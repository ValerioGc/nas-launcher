//! Discovers data filesystems and publishes only the wizard user's directories to Explorer.
//! This entry point runs as root from a dedicated oneshot unit, never from an HTTP handler.

pub mod crypt;
pub mod eject;
pub mod inventory;
pub mod network;

use inventory::{
    classify, contains_system, fstab_system_uuids, system_mount, Candidate, Device, Devices,
};

use crate::{
    http::error::ApiFailure,
    services::notifications::{self, Notice, Tone},
    utils::{self, Mode},
    widgets::services::tasks::{self, NewTask, TaskState, TaskUpdate},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{
    collections::HashSet,
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

const VOLUMES_ROOT: &str = "/srv/nas-volumes";
const EXPLORER_ROOT: &str = "/srv/storage";
const REGISTRY: &str = "/etc/nas-os/storage-volumes.tsv";
/// Drop-in recording the terminal account provisioned by the wizard.
const TERMINAL_DROPIN: &str = "/etc/systemd/system/nas-os-terminal@.service.d";
const GROUP: &str = "nas-storage";
const INITIALIZE_UNIT: &str = "nas-os-storage-initialize.service";
const INITIALIZE_TASK: &str = "initialize-storage";
const RENAME_UNIT: &str = "nas-os-storage-rename.service";
const SMART_UNIT: &str = "nas-os-storage-smart.service";
const SHARES_UNIT: &str = "nas-os-smb-shares.service";
/// Reconciliation time budget for automatic network-disk connections.
const NETWORK_BUDGET: Duration = Duration::from_secs(30);
static INITIALIZE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InitializeRequest {
    pub device: String,
    pub label: String,
    /// Disk identity captured during inventory, used to reject stale requests.
    pub identity: String,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitializeAccepted {
    pub task_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameRequest {
    pub device: String,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SmartRequest {
    pub device: String,
}

#[derive(Debug)]
struct Grant {
    id: String,
    name: String,
    path: PathBuf,
    /// `device`, `external` or `network`, as Explorer groups them.
    kind: &'static str,
}

fn output(program: &str, arguments: &[&str]) -> Result<String, String> {
    let result = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("{program}: {error}"))?;
    if !result.status.success() {
        return Err(format!(
            "{program}: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&result.stdout).trim().to_owned())
}

fn output_logged(program: &str, arguments: &[&str], log: &Path) -> Result<String, String> {
    use std::io::Write;

    let command = format!("$ {program} {}", arguments.join(" "));
    let append = |text: &str| {
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .and_then(|mut file| writeln!(file, "{text}"));
    };
    append(&command);

    let result = match Command::new(program).args(arguments).output() {
        Ok(result) => result,
        Err(error) => {
            append(&format!("spawn error: {error}\n"));
            return Err(format!("{program}: {error}"));
        }
    };
    append(&match result.status.code() {
        Some(code) => format!("exit code: {code}"),
        None => format!("exit status: {}", result.status),
    });
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    if !stdout.trim().is_empty() {
        append(&format!("stdout:\n{}", stdout.trim()));
    }
    if !stderr.trim().is_empty() {
        append(&format!("stderr:\n{}", stderr.trim()));
    }
    append("");
    if !result.status.success() {
        return Err(format!("{program}: {}", stderr.trim()));
    }
    Ok(stdout.trim().to_owned())
}

fn mount_field(path: &Path, field: &str) -> Option<String> {
    output(
        "findmnt",
        &[
            "--mountpoint",
            path.to_str()?,
            "--noheadings",
            "--output",
            field,
        ],
    )
    .ok()
    .filter(|value| !value.is_empty())
}

fn mounted_uuid(path: &Path) -> Option<String> {
    mount_field(path, "UUID")
}

fn prepare_directory(path: &Path, mode: u32, owner: &str) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|error| error.to_string())?;
    output("chown", &[owner, path.to_str().ok_or("invalid path")?])?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|error| error.to_string())
}

/// Checks whether ownership and mode are stored by the filesystem rather than mount options.
fn keeps_permissions(fstype: &str) -> bool {
    !matches!(fstype, "vfat" | "exfat" | "ntfs" | "ntfs3")
}

/// Mounts a candidate filesystem at `raw` using an existing verified root mount
/// or the filesystem UUID.
fn mount_raw(candidate: &Candidate, raw: &Path, uid: &str, gid: &str) -> Result<(), String> {
    if mount_field(raw, "SOURCE").is_some() {
        return Err(format!("{} has another filesystem mounted", raw.display()));
    }
    let target = raw.to_str().ok_or("invalid mountpoint")?;
    let Some(existing) = &candidate.mountpoint else {
        let options = if keeps_permissions(&candidate.fstype) {
            "nosuid,nodev".to_owned()
        } else {
            format!("nosuid,nodev,uid={uid},gid={gid},umask=0007")
        };
        let source = format!("UUID={}", candidate.uuid);
        return output("mount", &["-o", &options, &source, target]).map(|_| ());
    };
    if mounted_uuid(existing).as_deref() != Some(candidate.uuid.as_str())
        || (candidate.fstype != "btrfs" && mount_field(existing, "FSROOT").as_deref() != Some("/"))
    {
        return Err(format!(
            "{} is not the expected filesystem",
            existing.display()
        ));
    }
    let source = existing.to_str().ok_or("invalid mountpoint")?;
    output("mount", &["--bind", source, target]).map(|_| ())
}

/// The area of `user` on the raw filesystem, created when missing and checked to be writable by
/// the storage group.
fn user_area(
    raw: &Path,
    candidate: &Candidate,
    user: &str,
    uid: &str,
    gid: &str,
) -> Result<PathBuf, String> {
    let posix = keeps_permissions(&candidate.fstype);
    let users = raw.join(".nas-users");
    let home = users.join(user);
    if posix {
        prepare_directory(&users, 0o700, "root:root")?;
        prepare_directory(&home, 0o2770, &format!("{user}:{GROUP}"))?;
    } else {
        fs::create_dir_all(&home).map_err(|error| error.to_string())?;
    }
    let metadata = fs::metadata(&home).map_err(|error| error.to_string())?;
    if metadata.uid() != uid.parse::<u32>().map_err(|error| error.to_string())?
        || metadata.mode() & 0o700 != 0o700
        || metadata.gid() != gid.parse::<u32>().map_err(|error| error.to_string())?
        || metadata.mode() & 0o070 != 0o070
    {
        return Err(format!("{} is not writable by {GROUP}", home.display()));
    }
    Ok(home)
}

fn mount_candidate(
    candidate: &Candidate,
    user: &str,
    uid: &str,
    gid: &str,
) -> Result<Grant, String> {
    let raw = Path::new(VOLUMES_ROOT).join(&candidate.id);
    let visible = Path::new(EXPLORER_ROOT).join(&candidate.id);
    fs::create_dir_all(&raw).map_err(|error| error.to_string())?;
    if mounted_uuid(&raw).as_deref() != Some(candidate.uuid.as_str()) {
        mount_raw(candidate, &raw, uid, gid)?;
    }
    let raw_root = mount_field(&raw, "FSROOT").ok_or("raw filesystem has no root")?;
    if mounted_uuid(&raw).as_deref() != Some(candidate.uuid.as_str())
        || (candidate.fstype != "btrfs" && raw_root != "/")
    {
        return Err(format!("{} did not mount the expected UUID", raw.display()));
    }

    let home = user_area(&raw, candidate, user, uid, gid)?;
    fs::create_dir_all(&visible).map_err(|error| error.to_string())?;
    if mount_field(&visible, "SOURCE").is_none() {
        output(
            "mount",
            &[
                "--bind",
                home.to_str().ok_or("invalid user path")?,
                visible.to_str().ok_or("invalid view path")?,
            ],
        )?;
    }
    let expected_root = format!("{}/.nas-users/{user}", raw_root.trim_end_matches('/'));
    if mounted_uuid(&visible).as_deref() != Some(candidate.uuid.as_str())
        || mount_field(&visible, "FSROOT").as_deref() != Some(expected_root.as_str())
    {
        return Err(format!("{} is not the user's volume", visible.display()));
    }
    Ok(Grant {
        id: candidate.id.clone(),
        name: candidate.name.clone(),
        path: visible,
        kind: if candidate.external {
            "external"
        } else {
            "device"
        },
    })
}

/// Reads the provisioned terminal account from its drop-in without accessing the launcher
/// database.
async fn wizard_user(dropin: &Path) -> Result<Option<String>, String> {
    let text = match tokio::fs::read_to_string(dropin).await {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", dropin.display())),
    };
    let Some(user) = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("User="))
        .map(str::to_owned)
    else {
        return Ok(None);
    };
    if !user
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err("invalid wizard user".to_owned());
    }
    let account = output("getent", &["passwd", &user])?;
    if account.split(':').nth(4) != Some("nas-os terminal") {
        return Err("wizard user is not the expected OS account".to_owned());
    }
    Ok(Some(user))
}

fn publish(grants: &[Grant]) -> Result<bool, String> {
    let mut content = String::new();
    for grant in grants {
        content.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            grant.id,
            grant.name,
            grant.path.display(),
            grant.kind
        ));
    }
    let registry = std::env::var_os("NAS_STORAGE_REGISTRY")
        .map_or_else(|| PathBuf::from(REGISTRY), PathBuf::from);
    let registry = registry.as_path();
    if fs::read_to_string(registry).ok().as_deref() == Some(content.as_str()) {
        return Ok(false);
    }
    let temporary = registry.with_extension("tsv.new");
    fs::write(&temporary, content).map_err(|error| error.to_string())?;
    output(
        "chown",
        &[
            &format!("root:{GROUP}"),
            temporary.to_str().ok_or("invalid registry")?,
        ],
    )?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o640))
        .map_err(|error| error.to_string())?;
    fs::rename(temporary, registry).map_err(|error| error.to_string())?;
    Ok(true)
}

fn validate_initialize(input: &InitializeRequest) -> Result<(), String> {
    if input.device.is_empty()
        || input.device.len() > 64
        || !input
            .device
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        || input.device.starts_with('.')
    {
        return Err("invalid disk device".to_owned());
    }
    if input.label.trim() != input.label
        || input.label.is_empty()
        || input.label.len() > 16
        || !input
            .label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'-' | b'_'))
    {
        return Err("invalid filesystem label".to_owned());
    }
    if !inventory::valid_identity(&input.identity) {
        return Err("invalid disk identity".to_owned());
    }
    Ok(())
}

fn blank_disk(devices: &[Device], device: &str, expected_identity: &str) -> Result<(), String> {
    let expected = format!("/dev/{device}");
    let disk = devices
        .iter()
        .find(|item| {
            item.name.as_deref() == Some(device) && item.path.as_deref() == Some(&expected)
        })
        .ok_or("disk is no longer present")?;
    let mounted = disk
        .mountpoints
        .as_ref()
        .is_some_and(|paths| paths.iter().flatten().any(|path| !path.is_empty()));
    if disk.kind != "disk"
        || disk.ro.unwrap_or(false)
        || disk.fstype.as_ref().is_some_and(|value| !value.is_empty())
        || disk.uuid.as_ref().is_some_and(|value| !value.is_empty())
        || mounted
        || disk
            .children
            .as_ref()
            .is_some_and(|children| !children.is_empty())
    {
        return Err("the selected device is not an empty writable whole disk".to_owned());
    }
    if inventory::identity(disk) != expected_identity {
        return Err("the disk is not the one that was selected: list the disks again".to_owned());
    }
    Ok(())
}

fn inspect_blank_disk(input: &InitializeRequest) -> Result<(), String> {
    blank_disk(
        &inventory::list()?.blockdevices,
        &input.device,
        &input.identity,
    )
}

fn inspect_signatures(device: &str) -> Result<(), String> {
    let path = format!("/dev/{device}");
    let signatures = output(
        "wipefs",
        &["--no-act", "--noheadings", "--output", "TYPE", &path],
    )?;
    if !signatures.is_empty() {
        return Err(
            "the selected disk contains a partition table or filesystem signature".to_owned(),
        );
    }
    Ok(())
}

/// Directory for privileged storage requests and results, overridden by `NAS_STORAGE_STATE_DIR`
/// in tests.
pub(crate) fn state_file(name: &str) -> PathBuf {
    std::env::var_os("NAS_STORAGE_STATE_DIR")
        .map_or_else(|| PathBuf::from("/var/lib/nas-os"), PathBuf::from)
        .join(name)
}

fn request_path() -> PathBuf {
    std::env::var_os("NAS_STORAGE_INITIALIZE_REQUEST")
        .map(PathBuf::from)
        .unwrap_or_else(|| state_file("storage-initialize.json"))
}

fn error_path() -> PathBuf {
    std::env::var_os("NAS_STORAGE_INITIALIZE_ERROR")
        .map(PathBuf::from)
        .unwrap_or_else(|| state_file("storage-initialize.error"))
}

fn log_path() -> PathBuf {
    std::env::var_os("NAS_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/log/nas-os"))
        .join("initialize-storage.log")
}

fn rename_request_path() -> PathBuf {
    state_file("storage-rename.json")
}

fn rename_error_path() -> PathBuf {
    state_file("storage-rename.error")
}

fn rename_log_path() -> PathBuf {
    std::env::var_os("NAS_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/log/nas-os"))
        .join("rename-storage.log")
}

fn smart_request_path() -> PathBuf {
    state_file("storage-smart.json")
}

fn smart_result_path() -> PathBuf {
    state_file("storage-smart-result.json")
}

fn smart_error_path() -> PathBuf {
    state_file("storage-smart.error")
}

fn valid_device_name(device: &str) -> bool {
    !device.is_empty()
        && device.len() <= 64
        && device
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

fn validate_rename(input: &RenameRequest) -> Result<(), String> {
    if !valid_device_name(&input.device) {
        return Err("invalid device".to_owned());
    }
    if input.label.trim().is_empty()
        || input.label.len() > 16
        || input
            .label
            .chars()
            .any(|character| character.is_control() || character == '/')
    {
        return Err("invalid volume label".to_owned());
    }
    Ok(())
}

fn inspect_smart_target(device_name: &str) -> Result<(), String> {
    if !valid_device_name(device_name) {
        return Err("invalid device".to_owned());
    }
    let devices = inventory::list()?;
    let device = devices
        .blockdevices
        .iter()
        .find(|device| device.name.as_deref() == Some(device_name))
        .ok_or("device not found")?;
    if device.kind != "disk"
        || device.path.as_deref() != Some(format!("/dev/{device_name}").as_str())
        || device.ro.unwrap_or(false)
        || contains_system(device, &inventory::system_uuids())
    {
        return Err("S.M.A.R.T. is available only for a data disk".to_owned());
    }
    Ok(())
}

pub async fn smart_data(input: SmartRequest) -> Result<serde_json::Value, ApiFailure> {
    if !valid_device_name(&input.device) {
        return Err(ApiFailure::bad("invalid device"));
    }
    let path = smart_request_path();
    tokio::fs::write(
        &path,
        serde_json::to_vec(&input).map_err(|_| ApiFailure::bad("invalid request"))?,
    )
    .await
    .map_err(|_| ApiFailure::unavailable("cannot save S.M.A.R.T. request"))?;
    let result_path = smart_result_path();
    let error_path = smart_error_path();
    let _ = tokio::fs::remove_file(&result_path).await;
    let _ = tokio::fs::remove_file(&error_path).await;
    let outcome = utils::systemctl(&["start", SMART_UNIT], Duration::from_secs(30)).await;
    let _ = tokio::fs::remove_file(path).await;
    if let Err(fallback) = outcome {
        let reason = tokio::fs::read_to_string(&error_path)
            .await
            .ok()
            .filter(|reason| !reason.trim().is_empty())
            .unwrap_or(fallback);
        return Err(ApiFailure::unavailable(reason));
    }
    let bytes = tokio::fs::read(&result_path)
        .await
        .map_err(|_| ApiFailure::unavailable("S.M.A.R.T. result is unavailable"))?;
    serde_json::from_slice(&bytes)
        .map_err(|_| ApiFailure::unavailable("S.M.A.R.T. returned invalid data"))
}

fn read_smart(path: &Path) -> Result<(), String> {
    let input: SmartRequest = serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("cannot read S.M.A.R.T. request: {error}"))?,
    )
    .map_err(|error| format!("invalid S.M.A.R.T. request: {error}"))?;
    inspect_smart_target(&input.device)?;
    let device = format!("/dev/{}", input.device);
    let result = Command::new("smartctl")
        .args(["--all", "--json", &device])
        .output()
        .map_err(|error| format!("smartctl: {error}"))?;
    if result.stdout.is_empty() {
        return Err(format!(
            "smartctl: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(&result.stdout)
        .map_err(|error| format!("smartctl returned invalid JSON: {error}"))?;
    if value
        .get("smartctl")
        .and_then(|smartctl| smartctl.get("exit_status"))
        .and_then(serde_json::Value::as_i64)
        .is_some_and(|status| status & 0b11 != 0)
    {
        let reason = value
            .get("smartctl")
            .and_then(|smartctl| smartctl.get("messages"))
            .map(ToString::to_string)
            .unwrap_or_else(|| "S.M.A.R.T. is unavailable".to_owned());
        return Err(reason);
    }
    fs::write(
        smart_result_path(),
        serde_json::to_vec(&value).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

pub fn smart_reported(path: &Path) -> Result<(), String> {
    let result = read_smart(path);
    if let Err(reason) = &result {
        let _ = fs::write(smart_error_path(), reason);
    }
    result
}

fn inspect_rename_target(input: &RenameRequest) -> Result<(), String> {
    let fstab = fs::read_to_string("/etc/fstab").unwrap_or_default();
    renamable(&inventory::list()?.blockdevices, &input.device, &fstab)
}

/// Checks that a writable ext data volume can be renamed without breaking label-based fstab
/// entries.
fn renamable(devices: &[Device], name: &str, fstab: &str) -> Result<(), String> {
    let device = inventory::find(devices, name).ok_or("device not found")?;
    let expected = format!("/dev/{name}");
    if device.path.as_deref() != Some(expected.as_str())
        || device.ro.unwrap_or(false)
        || device
            .fstype
            .as_deref()
            .is_none_or(|value| !matches!(value, "ext2" | "ext3" | "ext4"))
        || contains_system(device, &inventory::fstab_system_uuids(fstab))
    {
        return Err("only a writable ext data volume can be renamed".to_owned());
    }
    if device
        .label
        .as_ref()
        .is_some_and(|label| inventory::fstab_labels(fstab).contains(label))
    {
        return Err("fstab mounts this volume by its label: change it there first".to_owned());
    }
    Ok(())
}

pub async fn rename_volume(db: &SqlitePool, input: RenameRequest) -> Result<(), ApiFailure> {
    validate_rename(&input).map_err(ApiFailure::bad)?;
    let target = input.clone();
    tokio::task::spawn_blocking(move || inspect_rename_target(&target))
        .await
        .map_err(|_| ApiFailure::unavailable("volume inspection failed"))?
        .map_err(ApiFailure::bad)?;
    let path = rename_request_path();
    let temporary = path.with_extension("json.new");
    tokio::fs::write(
        &temporary,
        serde_json::to_vec(&input)
            .map_err(|_| ApiFailure::unavailable("cannot prepare volume rename"))?,
    )
    .await
    .map_err(|_| ApiFailure::unavailable("cannot save volume rename request"))?;
    tokio::fs::rename(&temporary, &path)
        .await
        .map_err(|_| ApiFailure::unavailable("cannot save volume rename request"))?;
    let failure = rename_error_path();
    let _ = tokio::fs::remove_file(&failure).await;
    let outcome = utils::systemctl(&["start", RENAME_UNIT], Duration::from_secs(60)).await;
    let _ = tokio::fs::remove_file(path).await;
    if let Err(fallback) = outcome {
        let reason = tokio::fs::read_to_string(&failure)
            .await
            .ok()
            .filter(|reason| !reason.trim().is_empty())
            .unwrap_or(fallback);
        notifications::record(
            db,
            "nas-disk-manager",
            Tone::Error,
            Notice::new("diskRenameFailed", "Volume rename failed", reason.clone())
                .with("disk", input.device)
                .with("reason", reason.clone())
                .log(rename_log_path().display().to_string()),
        )
        .await;
        return Err(ApiFailure::unavailable(reason));
    }
    let _ = tokio::fs::remove_file(failure).await;
    Ok(())
}

pub async fn rename_reported(path: &Path) -> Result<(), String> {
    let input: RenameRequest = serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("cannot read rename request: {error}"))?,
    )
    .map_err(|error| format!("invalid rename request: {error}"))?;
    validate_rename(&input)?;
    inspect_rename_target(&input)?;
    let log = rename_log_path();
    if let Some(parent) = log.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(
        &log,
        format!(
            "Volume rename\nDevice: /dev/{}\nNew label: {}\n\n",
            input.device, input.label
        ),
    )
    .map_err(|error| error.to_string())?;
    let device = format!("/dev/{}", input.device);
    let result = match output_logged("e2label", &[&device, &input.label], &log) {
        Ok(_) => reconcile().await,
        Err(reason) => Err(reason),
    };
    if let Err(reason) = &result {
        let _ = fs::write(rename_error_path(), reason);
    }
    result
}

pub async fn start_initialize(
    db: &SqlitePool,
    input: InitializeRequest,
) -> Result<InitializeAccepted, ApiFailure> {
    let _guard = INITIALIZE_LOCK.lock().await;
    validate_initialize(&input).map_err(ApiFailure::bad)?;
    if !utils::is_real(Mode::Storage) {
        return Err(ApiFailure::unavailable(
            "Disk initialization is not available on this installation.",
        ));
    }
    if tasks::list(db)
        .await?
        .iter()
        .any(|task| task.id == INITIALIZE_TASK && task.state.active())
    {
        return Err(ApiFailure::bad(
            "another disk initialization is already running",
        ));
    }
    let request = input.clone();
    let inspected = tokio::task::spawn_blocking(move || inspect_blank_disk(&request))
        .await
        .map_err(|_| ApiFailure::unavailable("disk inspection failed"))?;
    if let Err(reason) = inspected {
        let log = log_path();
        if let Some(parent) = log.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let _ = tokio::fs::write(
            &log,
            format!(
                "Storage initialization\nDevice: /dev/{}\nStage: inventory validation\nFAILED\n{reason}\n",
                input.device
            ),
        )
        .await;
        notifications::record(
            db,
            "nas-disk-manager",
            Tone::Error,
            Notice::new(
                "diskInitializationFailed",
                "Disk initialization failed",
                format!("Initialization of {} failed ({reason}).", input.device),
            )
            .with("disk", input.device.clone())
            .with("reason", reason.clone())
            .log(log.display().to_string()),
        )
        .await;
        return Err(ApiFailure::bad(reason));
    }

    let path = request_path();
    let temporary = path.with_extension("json.new");
    let encoded = serde_json::to_vec(&input)
        .map_err(|_| ApiFailure::unavailable("cannot prepare disk initialization"))?;
    tokio::fs::write(&temporary, encoded)
        .await
        .map_err(|_| ApiFailure::unavailable("cannot save disk initialization request"))?;
    tokio::fs::rename(&temporary, &path)
        .await
        .map_err(|_| ApiFailure::unavailable("cannot save disk initialization request"))?;

    let task = tasks::register(
        db,
        NewTask {
            id: INITIALIZE_TASK.to_owned(),
            source: "nas-disk-manager".to_owned(),
            name: format!("Initialize {}", input.device),
            detail: "partitioning and formatting".to_owned(),
            progress: None,
            pausable: false,
            cancellable: false,
        },
    )
    .await?;
    let pool = db.clone();
    tokio::spawn(async move {
        let failure_path = error_path();
        let _ = tokio::fs::remove_file(&failure_path).await;
        let outcome =
            utils::systemctl(&["start", INITIALIZE_UNIT], Duration::from_secs(10 * 60)).await;
        let _ = tokio::fs::remove_file(request_path()).await;
        let (state, detail, progress) = match outcome {
            Ok(()) => (TaskState::Done, "ready for use".to_owned(), Some(100)),
            Err(fallback) => {
                let reason = tokio::fs::read_to_string(&failure_path)
                    .await
                    .ok()
                    .filter(|reason| !reason.trim().is_empty())
                    .unwrap_or(fallback);
                (TaskState::Failed, reason, None)
            }
        };
        let _ = tokio::fs::remove_file(failure_path).await;
        let _ = tasks::update(
            &pool,
            INITIALIZE_TASK,
            TaskUpdate {
                state: Some(state),
                detail: Some(detail),
                progress: Some(progress),
                ..TaskUpdate::default()
            },
        )
        .await;
    });
    Ok(InitializeAccepted { task_id: task.id })
}

/// One command of the initialization, written to the log of the operation when there is one.
fn run_step(program: &str, arguments: &[&str], log: Option<&Path>) -> Result<String, String> {
    match log {
        Some(log) => output_logged(program, arguments, log),
        None => output(program, arguments),
    }
}

/// Returns a disk partition only when exactly one partition is present.
fn single_partition(name: &str, device: &str) -> Option<String> {
    let disk = inventory::list()
        .ok()?
        .blockdevices
        .into_iter()
        .find(|item| item.name.as_deref() == Some(name) && item.path.as_deref() == Some(device))?;
    match disk.children.unwrap_or_default().as_slice() {
        [only] if only.kind == "part" => only.path.clone(),
        _ => None,
    }
}

async fn initialize_with_log(path: &Path, log: Option<&Path>) -> Result<(), String> {
    let input: InitializeRequest = serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("cannot read initialization request: {error}"))?,
    )
    .map_err(|error| format!("invalid initialization request: {error}"))?;
    validate_initialize(&input)?;
    inspect_blank_disk(&input)?;
    inspect_signatures(&input.device)?;
    let partition = partition(&input.device, log).await?;
    run_step("mkfs.ext4", &["-F", "-L", &input.label, &partition], log)?;
    reconcile().await
}

/// Gives the whole disk `name` one GPT partition, and returns it once the kernel shows it.
async fn partition(name: &str, log: Option<&Path>) -> Result<String, String> {
    let device = format!("/dev/{name}");
    let parted = [
        "--script", "--align", "optimal", &device, "mklabel", "gpt", "mkpart", "primary", "ext4",
        "0%", "100%",
    ];
    run_step("parted", &parted, log)?;
    run_step("partprobe", &[&device], log)?;
    run_step("udevadm", &["settle"], log)?;
    for _ in 0..40 {
        if let Some(partition) = single_partition(name, &device) {
            return Ok(partition);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err("the new partition did not appear".to_owned())
}

pub async fn initialize(path: &Path) -> Result<(), String> {
    initialize_with_log(path, None).await
}

pub async fn initialize_reported(path: &Path) -> Result<(), String> {
    let log = log_path();
    if let Some(parent) = log.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let request = fs::read_to_string(path).unwrap_or_else(|_| "unreadable request".to_owned());
    let _ = fs::write(
        &log,
        format!("Storage initialization\nRequest: {request}\nStages: validate, inspect signatures, partition GPT, format ext4, publish\n\n"),
    );
    let result = initialize_with_log(path, Some(&log)).await;
    if let Err(reason) = &result {
        let _ = std::fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .and_then(|mut file| {
                use std::io::Write;
                writeln!(file, "FAILED\n{reason}")
            });
        fs::write(error_path(), reason).map_err(|error| {
            format!("{reason}; cannot save initialization failure detail: {error}")
        })?;
    }
    result
}

/// Returns the provisioned account that owns data disks, if available.
pub async fn account_user() -> Result<Option<String>, String> {
    let dropin = std::env::var_os("NAS_TERMINAL_DROPIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(TERMINAL_DROPIN))
        .join("user.conf");
    wizard_user(&dropin).await
}

/// Returns the provisioned account, UID, and data-disk group GID, if available.
async fn owner() -> Result<Option<(String, String, String)>, String> {
    let Some(user) = account_user().await? else {
        return Ok(None);
    };
    let uid = output("id", &["-u", &user])?;
    let group = output("getent", &["group", GROUP])?;
    let gid = group
        .split(':')
        .nth(2)
        .ok_or("storage group has no gid")?
        .to_owned();
    Ok(Some((user, uid, gid)))
}

pub async fn reconcile() -> Result<(), String> {
    let Some((user, account, gid)) = owner().await? else {
        if publish(&[])? {
            volumes_changed();
        }
        return Ok(());
    };
    let gid = gid.as_str();
    fs::create_dir_all(VOLUMES_ROOT).map_err(|error| error.to_string())?;
    fs::create_dir_all(EXPLORER_ROOT).map_err(|error| error.to_string())?;
    let fstab = fs::read_to_string("/etc/fstab").map_err(|error| error.to_string())?;
    let mut devices = inventory::list()?;
    if crypt::root::open_automatic(&devices.blockdevices) {
        let _ = output("udevadm", &["settle"]);
        devices = inventory::list()?;
    }
    crypt::root::write_status_of(&devices.blockdevices);
    let candidates = classify(devices, &fstab_system_uuids(&fstab));
    let mut grants = Vec::new();
    let mut used = HashSet::new();
    for candidate in &candidates {
        if used.insert(&candidate.id) {
            match mount_candidate(candidate, &user, &account, gid) {
                Ok(grant) => grants.push(grant),
                Err(reason) => eprintln!("{} unavailable: {reason}", candidate.id),
            }
        }
    }
    let (shares, pending) = network::root::mounted_shares();
    grants.extend(shares);
    seal_unmounted(Path::new(VOLUMES_ROOT));
    seal_unmounted(Path::new(EXPLORER_ROOT));
    let mut changed = publish(&grants)?;
    // The disks of the machine are published before any network disk is waited for.
    let connected = network::root::connect_pending(&pending, (&account, gid), NETWORK_BUDGET);
    if !connected.is_empty() {
        grants.extend(connected);
        changed |= publish(&grants)?;
    }
    if changed {
        volumes_changed();
    }
    Ok(())
}

/// Restricts unmounted volume directories to root to prevent writes onto the system disk.
/// Requires a readable mount inventory and preserves folders on other filesystems.
fn seal_unmounted(root: &Path) {
    let (Ok(mountinfo), Ok(parent), Ok(entries)) = (
        fs::read_to_string("/proc/self/mountinfo"),
        fs::metadata(root),
        fs::read_dir(root),
    ) else {
        return;
    };
    let mounted = inventory::mountpoints(&mountinfo);
    for entry in entries.flatten() {
        let path = entry.path();
        if entry
            .metadata()
            .is_ok_and(|folder| folder.is_dir() && folder.dev() == parent.dev())
            && !mounted.contains(&path)
        {
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o000));
        }
    }
}

/// Explorer reads the volumes again, and the shares are applied again for the volumes now there.
fn volumes_changed() {
    let _ = output(
        "systemctl",
        &["try-restart", "nas-app@nas-file-manager.service"],
    );
    if fs::read_dir(state_file("smb-shares")).is_ok_and(|mut entries| entries.next().is_some()) {
        let _ = output("systemctl", &["start", "--no-block", SHARES_UNIT]);
    }
}

#[cfg(test)]
#[path = "../../tests/services/storage.rs"]
mod tests;
