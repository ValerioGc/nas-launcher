//! The one reading of the block devices of the machine: what belongs to the system, what is a data
//! volume, what may be initialized, and how a disk is told apart when the kernel gives its name to
//! another. The reconciliation, the root units and the inventory of the disk manager all use it.

use super::{output, EXPLORER_ROOT, VOLUMES_ROOT};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
};

const COLUMNS: &str =
    "NAME,PATH,MAJ:MIN,TYPE,SIZE,FSTYPE,PARTTYPE,UUID,LABEL,MOUNTPOINTS,RO,RM,TRAN,MODEL,SERIAL,WWN";

#[derive(Debug, Deserialize)]
pub(super) struct Devices {
    pub(super) blockdevices: Vec<Device>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct Device {
    pub(super) name: Option<String>,
    pub(super) path: Option<String>,
    #[serde(rename = "maj:min")]
    pub(super) number: Option<String>,
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) size: Option<u64>,
    pub(super) fstype: Option<String>,
    pub(super) parttype: Option<String>,
    pub(super) uuid: Option<String>,
    pub(super) label: Option<String>,
    pub(super) mountpoints: Option<Vec<Option<String>>>,
    pub(super) ro: Option<bool>,
    pub(super) rm: Option<bool>,
    pub(super) tran: Option<String>,
    pub(super) model: Option<String>,
    pub(super) serial: Option<String>,
    pub(super) wwn: Option<String>,
    pub(super) children: Option<Vec<Device>>,
}

/// A filesystem the reconciliation may publish to Explorer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Candidate {
    pub(super) id: String,
    pub(super) uuid: String,
    pub(super) name: String,
    pub(super) fstype: String,
    pub(super) mountpoint: Option<PathBuf>,
    pub(super) device: Option<String>,
    pub(super) external: bool,
}

/// A volume or a disk as the disk manager lists it.
#[derive(Debug, Clone, PartialEq, Serialize, utoipa::ToSchema)]
pub struct Disk {
    pub id: String,
    pub name: String,
    pub device: Option<String>,
    pub kind: &'static str,
    pub disk_type: String,
    pub size_bytes: u64,
    pub mount: Option<String>,
    pub filesystem: Option<String>,
    pub model: Option<String>,
    pub transport: Option<String>,
    pub removable: bool,
    pub read_only: bool,
    pub initializable: bool,
    pub parent: Option<String>,
    pub uuid: Option<String>,
    /// The disk holds the system: nothing on it may be erased.
    pub system_disk: bool,
    /// The identity of the whole disk, which an erasing request repeats.
    pub identity: String,
    /// `locked` or `open` for a LUKS container.
    pub encryption: Option<&'static str>,
    /// The keys of a LUKS container, as the root unit last saw them.
    pub keys: Option<super::crypt::Keys>,
}

/// The block devices as a tree, sizes in bytes.
pub(super) fn list() -> Result<Devices, String> {
    let listed = output(
        "lsblk",
        &["--json", "--tree", "--bytes", "--output", COLUMNS],
    )?;
    serde_json::from_str(&listed).map_err(|error| error.to_string())
}

pub(super) fn system_mount(path: &str) -> bool {
    if path.starts_with("/run/media/") {
        return false;
    }
    [
        "/", "/boot", "/efi", "/usr", "/var", "/home", "/etc", "/sys", "/proc", "/dev", "/run",
    ]
    .iter()
    .any(|mount| path == *mount || (*mount != "/" && path.starts_with(&format!("{mount}/"))))
}

pub(super) fn system_partition(parttype: Option<&str>) -> bool {
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

/// The block itself is the system: swap, a partition type of the system, a filesystem fstab
/// mounts for the system, or one mounted where the system lives.
fn is_system(device: &Device, system_uuids: &HashSet<String>) -> bool {
    device.fstype.as_deref() == Some("swap")
        || system_partition(device.parttype.as_deref())
        || device
            .uuid
            .as_ref()
            .is_some_and(|uuid| system_uuids.contains(&uuid.to_ascii_lowercase()))
        || device
            .mountpoints
            .iter()
            .flatten()
            .flatten()
            .any(|path| system_mount(path))
}

/// The block or one below it is the system.
pub(super) fn contains_system(device: &Device, system_uuids: &HashSet<String>) -> bool {
    is_system(device, system_uuids)
        || device
            .children
            .iter()
            .flatten()
            .any(|child| contains_system(child, system_uuids))
}

pub(super) fn external(device: &Device) -> bool {
    device.rm.unwrap_or(false)
        || device
            .tran
            .as_deref()
            .is_some_and(|transport| matches!(transport, "usb" | "mmc" | "firewire"))
}

pub(super) fn collect(
    device: Device,
    protected_uuids: &HashSet<String>,
    found: &mut Vec<Candidate>,
    parent_external: bool,
) -> bool {
    let external = parent_external || external(&device);
    let mut protected = is_system(&device, protected_uuids);
    let mountpoints = device.mountpoints.unwrap_or_default();
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

pub(super) fn classify(devices: Devices, protected_uuids: &HashSet<String>) -> Vec<Candidate> {
    let mut found = Vec::new();
    for device in devices.blockdevices {
        collect(device, protected_uuids, &mut found, false);
    }
    let mut sources = HashMap::<String, HashSet<Option<String>>>::new();
    let mut counts = HashMap::new();
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
    found
}

/// The fstab lines in use, as their source and their mountpoint.
fn fstab_entries(contents: &str) -> impl Iterator<Item = (&str, &str)> {
    contents.lines().filter_map(|line| {
        let mut fields = line.split_whitespace();
        let source = fields.next().filter(|source| !source.starts_with('#'))?;
        Some((source, fields.next()?))
    })
}

pub(super) fn fstab_system_uuids(contents: &str) -> HashSet<String> {
    fstab_entries(contents)
        .filter(|(_, mountpoint)| system_mount(mountpoint))
        .filter_map(|(source, _)| source.strip_prefix("UUID="))
        .map(str::to_ascii_lowercase)
        .collect()
}

/// The labels fstab mounts by: renaming one of them would leave its line without a filesystem.
pub(super) fn fstab_labels(contents: &str) -> HashSet<String> {
    fstab_entries(contents)
        .filter_map(|(source, _)| source.strip_prefix("LABEL="))
        .map(str::to_owned)
        .collect()
}

/// The filesystems fstab gives to the system, from the machine.
pub(super) fn system_uuids() -> HashSet<String> {
    fstab_system_uuids(&fs::read_to_string("/etc/fstab").unwrap_or_default())
}

/// What tells a whole disk apart: its WWN, its serial and its size.
pub(super) fn identity(disk: &Device) -> String {
    let field = |value: &Option<String>| value.as_deref().unwrap_or_default().trim().to_owned();
    format!(
        "{}/{}/{}",
        field(&disk.wwn),
        field(&disk.serial),
        disk.size.unwrap_or_default()
    )
}

pub(super) fn valid_identity(identity: &str) -> bool {
    (1..=256).contains(&identity.len()) && !identity.chars().any(char::is_control)
}

/// The block named `name` anywhere in the tree.
pub(super) fn find<'a>(devices: &'a [Device], name: &str) -> Option<&'a Device> {
    devices.iter().find_map(|device| {
        if device.name.as_deref() == Some(name) {
            Some(device)
        } else {
            find(device.children.as_deref().unwrap_or_default(), name)
        }
    })
}

/// The LUKS container whose UUID is `uuid` anywhere in the tree.
pub(super) fn find_luks<'a>(devices: &'a [Device], uuid: &str) -> Option<&'a Device> {
    devices.iter().find_map(|device| {
        if device.fstype.as_deref() == Some("crypto_LUKS")
            && device
                .uuid
                .as_deref()
                .is_some_and(|found| found.eq_ignore_ascii_case(uuid))
        {
            Some(device)
        } else {
            find_luks(device.children.as_deref().unwrap_or_default(), uuid)
        }
    })
}

/// Every LUKS container of the tree, with the blocks below it when it is open.
pub(super) fn containers(devices: &[Device]) -> Vec<&Device> {
    let mut found = Vec::new();
    for device in devices {
        if device.fstype.as_deref() == Some("crypto_LUKS") {
            found.push(device);
        }
        found.extend(containers(device.children.as_deref().unwrap_or_default()));
    }
    found
}

fn clean(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}

/// The disk a block belongs to, as the listing passes it down.
struct Parent<'a> {
    id: Option<&'a str>,
    external: bool,
    system_disk: bool,
    transport: Option<&'a str>,
    model: Option<&'a str>,
    identity: &'a str,
}

fn encryption_state(filesystem: Option<&str>, closed: bool) -> Option<&'static str> {
    match (filesystem, closed) {
        (Some("crypto_LUKS"), true) => Some("locked"),
        (Some("crypto_LUKS"), false) => Some("open"),
        _ => None,
    }
}

fn flatten(block: Device, parent: &Parent, system_uuids: &HashSet<String>, output: &mut Vec<Disk>) {
    let removable = block.rm.unwrap_or(false);
    let transport = clean(block.tran.clone()).or_else(|| parent.transport.map(str::to_owned));
    let model = clean(block.model.clone()).or_else(|| parent.model.map(str::to_owned));
    let external = parent.external || removable || transport.as_deref() == Some("usb");
    let kind = if external { "external" } else { "local" };
    let protected = contains_system(&block, system_uuids);
    let whole = block.kind == "disk";
    let system_disk = parent.system_disk || (whole && protected);
    let identity = if parent.id.is_none() {
        identity(&block)
    } else {
        parent.identity.to_owned()
    };
    let id = block.name.clone().unwrap_or_default();
    let children = block.children.unwrap_or_default();
    let mounted = block
        .mountpoints
        .iter()
        .flatten()
        .flatten()
        .any(|path| !path.trim().is_empty());
    let filesystem = clean(block.fstype);
    let initializable = whole
        && children.is_empty()
        && filesystem.is_none()
        && !mounted
        && !block.ro.unwrap_or(false)
        && !system_disk;
    if !protected
        && !(system_disk && block.kind == "part" && filesystem.is_none())
        && (!whole || children.is_empty())
        && matches!(
            block.kind.as_str(),
            "disk" | "part" | "lvm" | "crypt" | "raid"
        )
    {
        let label = clean(block.label);
        output.push(Disk {
            id: id.clone(),
            name: label
                .or_else(|| model.clone())
                .unwrap_or_else(|| id.clone()),
            device: clean(block.path),
            kind,
            disk_type: block.kind,
            size_bytes: block.size.unwrap_or_default(),
            mount: block.mountpoints.into_iter().flatten().find_map(clean),
            encryption: encryption_state(filesystem.as_deref(), children.is_empty()),
            filesystem,
            model: model.clone(),
            transport: transport.clone(),
            removable,
            read_only: block.ro.unwrap_or(false),
            initializable,
            parent: parent.id.map(str::to_owned),
            uuid: clean(block.uuid),
            system_disk,
            identity: identity.clone(),
            keys: None,
        });
    }
    let below = Parent {
        id: Some(&id),
        external,
        system_disk,
        transport: transport.as_deref(),
        model: model.as_deref(),
        identity: &identity,
    };
    for child in children {
        flatten(child, &below, system_uuids, output);
    }
}

/// A path of mountinfo, where a space, a tab, a newline and a backslash are written in octal.
fn unescape_mount(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 3 < bytes.len() {
            if let Some(byte) = std::str::from_utf8(&bytes[index + 1..index + 4])
                .ok()
                .and_then(|octal| u8::from_str_radix(octal, 8).ok())
            {
                output.push(byte);
                index += 4;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

/// Every mountpoint of a mountinfo.
pub(super) fn mountpoints(mountinfo: &str) -> HashSet<PathBuf> {
    mountinfo
        .lines()
        .filter_map(|line| line.split_whitespace().nth(4))
        .map(|path| PathBuf::from(unescape_mount(path)))
        .collect()
}

/// The mountpoints of the host for each device number: the launcher's own view hides some.
fn host_mounts(block: &mut Device, mounts: &HashMap<&str, Vec<Option<String>>>) {
    if let Some(number) = &block.number {
        block.mountpoints = Some(mounts.get(number.as_str()).cloned().unwrap_or_default());
    }
    for child in block.children.iter_mut().flatten() {
        host_mounts(child, mounts);
    }
}

/// The disks and volumes of `devices`, the mountpoints taken from `mountinfo`.
pub(super) fn inventory(
    devices: Devices,
    mountinfo: &str,
    system_uuids: &HashSet<String>,
) -> Vec<Disk> {
    let mut mounts: HashMap<&str, Vec<Option<String>>> = HashMap::new();
    for line in mountinfo.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if let (Some(number), Some(path)) = (fields.get(2), fields.get(4)) {
            mounts
                .entry(number)
                .or_default()
                .push(Some(unescape_mount(path)));
        }
    }
    let top = Parent {
        id: None,
        external: false,
        system_disk: false,
        transport: None,
        model: None,
        identity: "",
    };
    let mut disks = Vec::new();
    for mut block in devices.blockdevices {
        host_mounts(&mut block, &mounts);
        flatten(block, &top, system_uuids, &mut disks);
    }
    disks
}

/// The disks of the machine of one kind (`local` or `external`), or all of them.
pub fn disks(kind: Option<&str>) -> Result<Vec<Disk>, String> {
    let devices = list()?;
    let mountinfo = fs::read_to_string("/proc/1/mountinfo")
        .or_else(|_| fs::read_to_string("/proc/self/mountinfo"))
        .map_err(|error| format!("cannot read the mounts of the system: {error}"))?;
    let keys = super::crypt::keys();
    Ok(inventory(devices, &mountinfo, &system_uuids())
        .into_iter()
        .filter(|disk| kind.is_none_or(|kind| disk.kind == kind))
        .map(|mut disk| {
            if disk.encryption.is_some() {
                disk.keys = disk
                    .uuid
                    .as_ref()
                    .and_then(|uuid| keys.get(&uuid.to_ascii_lowercase()).copied());
            }
            disk
        })
        .collect())
}

#[cfg(test)]
#[path = "../../../../tests/services/storage/inventory.rs"]
mod tests;
