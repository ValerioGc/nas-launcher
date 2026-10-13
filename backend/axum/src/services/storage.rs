//! Discovers data filesystems and publishes only the wizard user's directories to Explorer.
//! This entry point runs as root from a dedicated oneshot unit, never from an HTTP handler.

pub mod eject;

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
/// Where `terminal-user.sh`, as root, names the user it created for the setup wizard.
const TERMINAL_DROPIN: &str = "/etc/systemd/system/nas-os-terminal@.service.d";
const GROUP: &str = "nas-storage";
const INITIALIZE_UNIT: &str = "nas-os-storage-initialize.service";
const INITIALIZE_TASK: &str = "initialize-storage";
const RENAME_UNIT: &str = "nas-os-storage-rename.service";
const SMART_UNIT: &str = "nas-os-storage-smart.service";
static INITIALIZE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Deserialize)]
struct Devices {
    blockdevices: Vec<Device>,
}

#[derive(Debug, Clone, Deserialize)]
struct Device {
    name: Option<String>,
    path: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    fstype: Option<String>,
    parttype: Option<String>,
    uuid: Option<String>,
    label: Option<String>,
    mountpoints: Option<Vec<Option<String>>>,
    ro: Option<bool>,
    rm: Option<bool>,
    tran: Option<String>,
    children: Option<Vec<Device>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InitializeRequest {
    pub device: String,
    pub label: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    id: String,
    uuid: String,
    name: String,
    fstype: String,
    mountpoint: Option<PathBuf>,
    device: Option<String>,
    external: bool,
}

#[derive(Debug)]
struct Grant {
    id: String,
    name: String,
    path: PathBuf,
    external: bool,
}

fn system_mount(path: &str) -> bool {
    if path.starts_with("/run/media/") {
        return false;
    }
    [
        "/", "/boot", "/efi", "/usr", "/var", "/home", "/etc", "/sys", "/proc", "/dev", "/run",
    ]
    .iter()
    .any(|mount| path == *mount || (*mount != "/" && path.starts_with(&format!("{mount}/"))))
}

fn system_partition(parttype: Option<&str>) -> bool {
    parttype.is_some_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "c12a7328-f81f-11d2-ba4b-00a0c93ec93b" | // EFI system
        "21686148-6449-6e6f-744e-656564454649" | // BIOS boot
        "0657fd6d-a4ab-43c4-84e5-0933c84b4f4f" | // swap
        "4f68bce3-e8cd-4db1-96e7-fbcaf984b709" | // x86-64 root
        "44479540-f297-41b2-9af7-d131d5f0458a" | // x86 root
        "4d21b016-b534-45c2-a9fb-5c16e091fd2d" | // /var
        "933ac7e1-2eb4-4f13-b844-0e14e2aef915" | // /home
        "de94bba4-06d1-4d40-a16a-bfd50179d6ac" | // Windows recovery
        "0x27" // MBR recovery
        )
    })
}

fn supported(fstype: &str) -> bool {
    matches!(
        fstype,
        "ext2" | "ext3" | "ext4" | "xfs" | "btrfs" | "vfat" | "exfat" | "ntfs" | "ntfs3"
    )
}

fn collect(
    device: Device,
    protected_uuids: &HashSet<String>,
    found: &mut Vec<Candidate>,
    parent_external: bool,
) -> bool {
    let external = parent_external
        || device.rm.unwrap_or(false)
        || device
            .tran
            .as_deref()
            .is_some_and(|transport| matches!(transport, "usb" | "mmc" | "firewire"));
    let mountpoints = device.mountpoints.unwrap_or_default();
    let mut protected = device.fstype.as_deref() == Some("swap")
        || system_partition(device.parttype.as_deref())
        || device
            .uuid
            .as_ref()
            .is_some_and(|uuid| protected_uuids.contains(&uuid.to_ascii_lowercase()))
        || mountpoints.iter().flatten().any(|path| system_mount(path));
    let children = device.children.unwrap_or_default();
    let is_leaf = children.is_empty();
    for child in children {
        protected |= collect(child, protected_uuids, found, external);
    }
    if protected || !is_leaf || device.ro.unwrap_or(false) {
        return protected;
    }
    if !matches!(device.kind.as_str(), "disk" | "part" | "lvm" | "crypt")
        && !device.kind.starts_with("raid")
    {
        return false;
    }
    let (Some(uuid), Some(fstype)) = (device.uuid, device.fstype) else {
        return false;
    };
    if !supported(&fstype)
        || uuid.is_empty()
        || !uuid
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return false;
    }
    let id = format!("v-{}", uuid.to_ascii_lowercase());
    let name = device
        .label
        .filter(|label| !label.trim().is_empty())
        .unwrap_or_else(|| format!("Disk {uuid}"));
    let name = name
        .chars()
        .filter(|character| !character.is_control())
        .take(80)
        .collect();
    let mountpoint = mountpoints
        .into_iter()
        .flatten()
        .find(|path| {
            path.starts_with('/')
                && !system_mount(path)
                && !path.starts_with(EXPLORER_ROOT)
                && !path.starts_with(VOLUMES_ROOT)
        })
        .map(PathBuf::from);
    found.push(Candidate {
        id,
        uuid,
        name,
        fstype,
        mountpoint,
        device: device.path,
        external,
    });
    false
}

fn classify_with_system_uuids(
    json: &str,
    protected_uuids: &HashSet<String>,
) -> Result<Vec<Candidate>, String> {
    let devices: Devices = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let mut found = Vec::new();
    for device in devices.blockdevices {
        collect(device, protected_uuids, &mut found, false);
    }
    let mut sources = std::collections::HashMap::<String, HashSet<Option<String>>>::new();
    let mut counts = std::collections::HashMap::new();
    for candidate in &found {
        sources
            .entry(candidate.id.clone())
            .or_default()
            .insert(candidate.device.clone());
        *counts.entry(candidate.id.clone()).or_insert(0_usize) += 1;
    }
    found.retain(|candidate| {
        sources
            .get(&candidate.id)
            .is_some_and(|devices| devices.len() == 1)
            && (candidate.device.is_some() || counts.get(&candidate.id) == Some(&1))
    });
    found.sort_by(|left, right| left.id.cmp(&right.id));
    found.dedup_by(|left, right| left.id == right.id);
    Ok(found)
}

fn fstab_system_uuids(contents: &str) -> HashSet<String> {
    contents
        .lines()
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if fields.len() < 2 || fields[0].starts_with('#') || !system_mount(fields[1]) {
                return None;
            }
            fields[0]
                .strip_prefix("UUID=")
                .map(|uuid| uuid.to_ascii_lowercase())
        })
        .collect()
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

/// Whether the filesystem keeps owners and modes: the FAT and NTFS family do not, and take them
/// from the mount options instead.
fn keeps_permissions(fstype: &str) -> bool {
    !matches!(fstype, "vfat" | "exfat" | "ntfs" | "ntfs3")
}

/// Mounts the filesystem of the candidate on `raw`: a bind of where the system already mounted
/// it, checked to be that filesystem from its root, or a mount by UUID.
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
        external: candidate.external,
    })
}

/// The user of the setup wizard, as the drop-in of the terminal names it: none until the wizard
/// created it. The launcher's database is not read, since this runs as root in another process.
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
            if grant.external { "external" } else { "device" }
        ));
    }
    let registry = Path::new(REGISTRY);
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
    Ok(())
}

fn blank_disk(json: &str, device: &str) -> Result<(), String> {
    let listed: Devices = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let expected = format!("/dev/{device}");
    let disk = listed
        .blockdevices
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
    Ok(())
}

fn inspect_blank_disk(device: &str) -> Result<(), String> {
    let listed = output(
        "lsblk",
        &[
            "--json",
            "--tree",
            "--output",
            "NAME,PATH,TYPE,FSTYPE,PARTTYPE,UUID,LABEL,MOUNTPOINTS,RO,RM,TRAN",
        ],
    )?;
    blank_disk(&listed, device)
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

/// Where the requests to the root units and their answers wait: `/var/lib/nas-os`, which the
/// tests move with `NAS_STORAGE_STATE_DIR`.
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
    let listed = output(
        "lsblk",
        &[
            "--json",
            "--tree",
            "--output",
            "NAME,PATH,TYPE,FSTYPE,PARTTYPE,MOUNTPOINTS,RO",
        ],
    )?;
    let devices: Devices = serde_json::from_str(&listed).map_err(|error| error.to_string())?;
    let device = devices
        .blockdevices
        .iter()
        .find(|device| device.name.as_deref() == Some(device_name))
        .ok_or("device not found")?;
    if device.kind != "disk"
        || device.path.as_deref() != Some(format!("/dev/{device_name}").as_str())
        || device.ro.unwrap_or(false)
        || device_is_protected(device)
    {
        return Err("S.M.A.R.T. is available only for a data disk".to_owned());
    }
    Ok(())
}

fn device_is_protected(device: &Device) -> bool {
    device.fstype.as_deref() == Some("swap")
        || system_partition(device.parttype.as_deref())
        || device
            .mountpoints
            .as_ref()
            .is_some_and(|paths| paths.iter().flatten().any(|path| system_mount(path)))
        || device
            .children
            .as_ref()
            .is_some_and(|children| children.iter().any(device_is_protected))
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
    let listed = output(
        "lsblk",
        &[
            "--json",
            "--tree",
            "--output",
            "NAME,PATH,TYPE,FSTYPE,PARTTYPE,MOUNTPOINTS,RO",
        ],
    )?;
    let devices: Devices = serde_json::from_str(&listed).map_err(|error| error.to_string())?;
    fn find<'a>(devices: &'a [Device], name: &str) -> Option<&'a Device> {
        for device in devices {
            if device.name.as_deref() == Some(name) {
                return Some(device);
            }
            if let Some(found) = find(device.children.as_deref().unwrap_or_default(), name) {
                return Some(found);
            }
        }
        None
    }
    let device = find(&devices.blockdevices, &input.device).ok_or("device not found")?;
    let expected = format!("/dev/{}", input.device);
    if device.path.as_deref() != Some(expected.as_str())
        || device.ro.unwrap_or(false)
        || device
            .fstype
            .as_deref()
            .is_none_or(|value| !matches!(value, "ext2" | "ext3" | "ext4"))
        || device
            .mountpoints
            .as_ref()
            .is_some_and(|paths| paths.iter().flatten().any(|path| system_mount(path)))
        || system_partition(device.parttype.as_deref())
    {
        return Err("only a writable ext data volume can be renamed".to_owned());
    }
    Ok(())
}

pub async fn rename_volume(db: &SqlitePool, input: RenameRequest) -> Result<(), ApiFailure> {
    validate_rename(&input).map_err(ApiFailure::bad)?;
    inspect_rename_target(&input).map_err(ApiFailure::bad)?;
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
    let device = input.device.clone();
    let inspected = tokio::task::spawn_blocking(move || inspect_blank_disk(&device))
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

/// The only partition of the disk, once the kernel shows it: none while the disk has none, or
/// more than one.
fn single_partition(name: &str, device: &str) -> Option<String> {
    let listed = output("lsblk", &["--json", "--tree", "--output", "NAME,PATH,TYPE"]).ok()?;
    let devices = serde_json::from_str::<Devices>(&listed).ok()?;
    let disk = devices
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
    inspect_blank_disk(&input.device)?;
    inspect_signatures(&input.device)?;

    let device = format!("/dev/{}", input.device);
    let parted = [
        "--script", "--align", "optimal", &device, "mklabel", "gpt", "mkpart", "primary", "ext4",
        "0%", "100%",
    ];
    run_step("parted", &parted, log)?;
    run_step("partprobe", &[&device], log)?;
    run_step("udevadm", &["settle"], log)?;

    let mut partition = None;
    for _ in 0..40 {
        partition = single_partition(&input.device, &device);
        if partition.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let partition = partition.ok_or("the new partition did not appear")?;
    run_step("mkfs.ext4", &["-F", "-L", &input.label, &partition], log)?;
    reconcile().await
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

/// The user of the machine the account of the setup wizard has, which owns the data disks: none
/// before the wizard made it.
pub async fn account_user() -> Result<Option<String>, String> {
    let dropin = std::env::var_os("NAS_TERMINAL_DROPIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(TERMINAL_DROPIN))
        .join("user.conf");
    wizard_user(&dropin).await
}

pub async fn reconcile() -> Result<(), String> {
    let user = account_user().await?;
    if user.is_none() {
        if publish(&[])? {
            let _ = output(
                "systemctl",
                &["try-restart", "nas-app@nas-file-manager.service"],
            );
        }
        return Ok(());
    }
    let user = user.ok_or("wizard user missing")?;
    let account = output("id", &["-u", &user])?;
    let group = output("getent", &["group", GROUP])?;
    let gid = group.split(':').nth(2).ok_or("storage group has no gid")?;
    fs::create_dir_all(VOLUMES_ROOT).map_err(|error| error.to_string())?;
    fs::create_dir_all(EXPLORER_ROOT).map_err(|error| error.to_string())?;
    let listed = output(
        "lsblk",
        &[
            "--json",
            "--tree",
            "--output",
            "NAME,PATH,TYPE,FSTYPE,PARTTYPE,UUID,LABEL,MOUNTPOINTS,RO,RM,TRAN",
        ],
    )?;
    let fstab = fs::read_to_string("/etc/fstab").map_err(|error| error.to_string())?;
    let candidates = classify_with_system_uuids(&listed, &fstab_system_uuids(&fstab))?;
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
    if publish(&grants)? {
        let _ = output(
            "systemctl",
            &["try-restart", "nas-app@nas-file-manager.service"],
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../tests/services/storage.rs"]
mod tests;
