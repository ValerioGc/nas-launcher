//! The root half of encryption: `nas-os-axum --storage-crypt` reads one request on its standard
//! input, the connection of `nas-os-storage-crypt.socket`, checks it again against the machine and
//! writes one answer on its standard output. The reconciliation opens with it the containers that
//! have an automatic key, and writes the keys of every container for the launcher.

use super::{
    held_path, status_path, validate, Answer, Keys, Request, AUTOMATIC_SLOT, PASSWORD_SLOT,
    RECOVERY_SLOT,
};
use crate::services::storage::{
    contains_system,
    eject::mountpoints,
    inventory::{self, Device},
    network::{self, NetworkRequest},
    output, output_logged, partition, reconcile, system_mount,
};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

const MAPPER_PREFIX: &str = "nas-luks-";
const REQUEST_LIMIT: u64 = 64 * 1024;

fn setting(name: &str, default: &str) -> PathBuf {
    std::env::var_os(name).map_or_else(|| PathBuf::from(default), PathBuf::from)
}

fn log_path() -> PathBuf {
    setting("NAS_LOG_DIR", "/var/log/nas-os").join("storage-crypt.log")
}

/// The encrypted credential of the automatic key of a container.
fn credential(uuid: &str) -> PathBuf {
    credstore().join(credential_name(uuid))
}

/// Where the encrypted credentials of systemd live.
pub(in crate::services::storage) fn credstore() -> PathBuf {
    setting("NAS_CREDSTORE", "/etc/credstore.encrypted")
}

fn credential_name(uuid: &str) -> String {
    format!("nas-os.luks-{}", uuid.to_ascii_lowercase())
}

fn mapper(uuid: &str) -> String {
    format!("{MAPPER_PREFIX}{}", uuid.to_ascii_lowercase())
}

/// A secret in a file of the runtime directory of the unit, a tmpfs only root reads: overwritten
/// and removed when dropped.
pub(in crate::services::storage) struct Secret(pub(in crate::services::storage) PathBuf);

impl Secret {
    pub(in crate::services::storage) fn path(name: &str) -> Result<PathBuf, String> {
        let directory = std::env::var_os("RUNTIME_DIRECTORY")
            .and_then(|value| {
                value
                    .to_str()
                    .and_then(|value| value.split(':').next().map(PathBuf::from))
            })
            .filter(|directory| directory.is_dir())
            .ok_or("no runtime directory for the keys")?;
        Ok(directory.join(name))
    }

    pub(in crate::services::storage) fn write(name: &str, bytes: &[u8]) -> Result<Self, String> {
        let path = Self::path(name)?;
        let _ = fs::remove_file(&path);
        let secret = Self(path);
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&secret.0)
            .and_then(|mut file| file.write_all(bytes))
            .map_err(|error| format!("cannot keep the key: {error}"))?;
        Ok(secret)
    }

    /// A file another program writes, removed as a secret.
    fn adopt(name: &str) -> Result<Self, String> {
        let path = Self::path(name)?;
        let _ = fs::remove_file(&path);
        Ok(Self(path))
    }

    pub(in crate::services::storage) fn arg(&self) -> &str {
        self.0.to_str().unwrap_or_default()
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        if let Ok(length) = fs::metadata(&self.0).map(|metadata| metadata.len()) {
            let _ = fs::OpenOptions::new()
                .write(true)
                .open(&self.0)
                .and_then(|mut file| file.write_all(&vec![0; length.try_into().unwrap_or(0)]));
        }
        let _ = fs::remove_file(&self.0);
    }
}

pub(in crate::services::storage) fn random(length: usize) -> Result<Vec<u8>, String> {
    let mut bytes = vec![0; length];
    fs::File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut bytes))
        .map_err(|error| format!("no random source: {error}"))?;
    Ok(bytes)
}

/// A recovery key: 32 random bytes in hexadecimal, in groups of eight to be copied by hand.
fn recovery_key() -> Result<String, String> {
    let hex: String = random(32)?
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(hex
        .as_bytes()
        .chunks(8)
        .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
        .collect::<Vec<_>>()
        .join("-"))
}

pub(super) fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let triple = chunk
            .iter()
            .enumerate()
            .fold(0_u32, |value, (index, byte)| {
                value | (u32::from(*byte) << (16 - 8 * index))
            });
        for index in 0..4 {
            if index <= chunk.len() {
                let sextet = (triple >> (18 - 6 * index)) & 0x3f;
                text.push(char::from(ALPHABET[sextet as usize]));
            } else {
                text.push('=');
            }
        }
    }
    text
}

/// What a program writes, as bytes: a key is binary and must come through untouched.
pub(in crate::services::storage) fn raw_output(
    program: &str,
    arguments: &[&str],
) -> Result<Vec<u8>, String> {
    let result = std::process::Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("{program}: {error}"))?;
    if !result.status.success() {
        return Err(format!(
            "{program}: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    Ok(result.stdout)
}

fn cryptsetup(arguments: &[&str]) -> Result<String, String> {
    output_logged("cryptsetup", arguments, &log_path())
}

/// The UUIDs /etc/crypttab opens for the system.
fn crypttab_uuids() -> HashSet<String> {
    fs::read_to_string(setting("NAS_CRYPTTAB", "/etc/crypttab"))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            fields.next().filter(|name| !name.starts_with('#'))?;
            fields.next()?.strip_prefix("UUID=")
        })
        .map(str::to_ascii_lowercase)
        .collect()
}

/// Whether a container belongs to the system: it holds a filesystem of the system, or crypttab
/// opens it at boot.
fn system_container(device: &Device, system: &HashSet<String>, crypttab: &HashSet<String>) -> bool {
    contains_system(device, system)
        || device
            .uuid
            .as_ref()
            .is_some_and(|uuid| crypttab.contains(&uuid.to_ascii_lowercase()))
}

/// The container with this UUID, read again from the machine: never one of the system.
fn container(uuid: &str) -> Result<(Device, String), String> {
    let devices = inventory::list()?;
    let device = inventory::find_luks(&devices.blockdevices, uuid)
        .ok_or("the encrypted volume is not connected")?
        .clone();
    if system_container(&device, &inventory::system_uuids(), &crypttab_uuids()) {
        return Err("the encrypted volume belongs to the system".to_owned());
    }
    let path = device
        .path
        .clone()
        .ok_or("the encrypted volume has no device")?;
    Ok((device, path))
}

/// The key slots in use, from the metadata of LUKS2.
fn slots(path: &str) -> Result<Vec<u8>, String> {
    let metadata = output("cryptsetup", &["luksDump", "--dump-json-metadata", path])?;
    slots_of(&metadata)
}

pub(super) fn slots_of(metadata: &str) -> Result<Vec<u8>, String> {
    let value: serde_json::Value =
        serde_json::from_str(metadata).map_err(|_| "unreadable LUKS metadata".to_owned())?;
    let mut slots: Vec<u8> = value
        .get("keyslots")
        .and_then(serde_json::Value::as_object)
        .ok_or("LUKS metadata without key slots")?
        .keys()
        .filter_map(|slot| slot.parse().ok())
        .collect();
    slots.sort_unstable();
    Ok(slots)
}

fn opens(path: &str, key: &Secret, slot: Option<u8>) -> bool {
    let slot = slot.map(|slot| slot.to_string());
    let mut arguments = vec!["open", "--test-passphrase", "--key-file", key.arg()];
    if let Some(slot) = &slot {
        arguments.extend(["--key-slot", slot]);
    }
    arguments.push(path);
    output("cryptsetup", &arguments).is_ok()
}

/// A random key needs no slow derivation: it has the strength of its bytes already.
const FAST: [&str; 4] = ["--pbkdf", "pbkdf2", "--pbkdf-force-iterations", "1000"];

fn add(path: &str, existing: &Secret, new: &Secret, slot: u8, fast: bool) -> Result<(), String> {
    let slot = slot.to_string();
    let mut arguments = vec![
        "luksAddKey",
        "--batch-mode",
        "--key-file",
        existing.arg(),
        "--new-key-slot",
        &slot,
    ];
    if fast {
        arguments.extend(FAST);
    }
    arguments.extend([path, new.arg()]);
    cryptsetup(&arguments).map(|_| ())
}

fn kill(path: &str, slot: u8) -> Result<(), String> {
    cryptsetup(&["luksKillSlot", "--batch-mode", path, &slot.to_string()]).map(|_| ())
}

/// Puts `new` in `slot`, with `current`, which opens the container: a slot `current` opens is
/// changed in place, another one is wiped first, never leaving the container without the key the
/// request came with.
fn replace(path: &str, slot: u8, current: &Secret, new: &Secret, fast: bool) -> Result<(), String> {
    if !slots(path)?.contains(&slot) {
        return add(path, current, new, slot, fast);
    }
    if opens(path, current, Some(slot)) {
        let slot = slot.to_string();
        let mut arguments = vec![
            "luksChangeKey",
            "--batch-mode",
            "--key-file",
            current.arg(),
            "--key-slot",
            &slot,
        ];
        if fast {
            arguments.extend(FAST);
        }
        arguments.extend([path, new.arg()]);
        return cryptsetup(&arguments).map(|_| ());
    }
    kill(path, slot)?;
    add(path, current, new, slot, fast)
}

fn load_automatic(uuid: &str) -> Result<Secret, String> {
    let source = credential(uuid);
    if !source.is_file() {
        return Err("the volume has no automatic key".to_owned());
    }
    let key = raw_output(
        "systemd-creds",
        &[
            "decrypt",
            &format!("--name={}", credential_name(uuid)),
            source.to_str().ok_or("invalid credential path")?,
            "-",
        ],
    )?;
    Secret::write("automatic", &key)
}

fn save_automatic(uuid: &str, key: &Secret) -> Result<(), String> {
    let sealed = raw_output(
        "systemd-creds",
        &[
            "encrypt",
            &format!("--name={}", credential_name(uuid)),
            key.arg(),
            "-",
        ],
    )?;
    let target = credential(uuid);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = target.with_extension("new");
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)
        .and_then(|mut file| file.write_all(&sealed))
        .map_err(|error| format!("cannot keep the automatic key: {error}"))?;
    fs::rename(&temporary, &target).map_err(|error| error.to_string())
}

/// The key the request gave, or the automatic one, checked to open the container.
fn current_key(uuid: &str, path: &str, given: Option<&String>) -> Result<Secret, String> {
    let key = match given {
        Some(password) => Secret::write("current", password.as_bytes())?,
        None => load_automatic(uuid)?,
    };
    if opens(path, &key, None) {
        Ok(key)
    } else {
        Err("the password does not open the volume".to_owned())
    }
}

pub(in crate::services::storage) fn held() -> HashSet<String> {
    fs::read_to_string(held_path())
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|uuid| super::valid_uuid(uuid))
        .map(str::to_ascii_lowercase)
        .collect()
}

pub(in crate::services::storage) fn set_held(uuid: &str, closed: bool) -> Result<(), String> {
    let mut list = held();
    let uuid = uuid.to_ascii_lowercase();
    if closed {
        list.insert(uuid);
    } else {
        list.remove(&uuid);
    }
    let mut lines: Vec<_> = list.into_iter().collect();
    lines.sort();
    let text: String = lines.iter().map(|uuid| format!("{uuid}\n")).collect();
    fs::write(held_path(), text).map_err(|error| error.to_string())
}

/// The processes that keep something open under one of `mountpoints`: their unit when they run in
/// one of the NAS, their name otherwise.
pub(in crate::services::storage) fn users_of(proc: &Path, mountpoints: &[String]) -> Vec<String> {
    let under = |target: &Path| {
        mountpoints.iter().any(|mountpoint| {
            target.starts_with(mountpoint) && !mountpoint.is_empty() && mountpoint != "/"
        })
    };
    let mut found = Vec::new();
    let own = std::process::id().to_string();
    for entry in fs::read_dir(proc).into_iter().flatten().flatten() {
        let pid = entry.file_name().to_string_lossy().into_owned();
        if pid == own || !pid.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let process = entry.path();
        let mut links: Vec<PathBuf> = ["cwd", "root", "exe"]
            .iter()
            .map(|name| process.join(name))
            .collect();
        links.extend(
            fs::read_dir(process.join("fd"))
                .into_iter()
                .flatten()
                .flatten()
                .map(|fd| fd.path()),
        );
        if !links
            .iter()
            .filter_map(|link| fs::read_link(link).ok())
            .any(|target| under(&target))
        {
            continue;
        }
        let unit = fs::read_to_string(process.join("cgroup"))
            .ok()
            .and_then(|cgroup| {
                cgroup
                    .lines()
                    .next()
                    .and_then(|line| line.rsplit('/').next())
                    .filter(|unit| unit.ends_with(".service"))
                    .map(str::to_owned)
            });
        let name = unit.unwrap_or_else(|| {
            fs::read_to_string(process.join("comm"))
                .map(|comm| comm.trim().to_owned())
                .unwrap_or(pid)
        });
        if !found.contains(&name) {
            found.push(name);
        }
    }
    found
}

/// Unmounts what is mounted from `device` and below it, the views of Explorer first, after making
/// sure nothing uses it, then closes its open containers.
fn release(device: &Device) -> Result<(), String> {
    let mounted = mountpoints(device);
    if let Some(system) = mounted.iter().find(|path| system_mount(path)) {
        return Err(format!("{system} belongs to the system"));
    }
    let users = users_of(Path::new("/proc"), &mounted);
    if !users.is_empty() {
        return Err(format!("the volume is in use by {}", users.join(", ")));
    }
    let log = log_path();
    for mountpoint in &mounted {
        output_logged("umount", &[mountpoint], &log)?;
    }
    close_below(device)
}

fn close_below(device: &Device) -> Result<(), String> {
    for child in device.children.iter().flatten() {
        close_below(child)?;
        if child.kind == "crypt" {
            if let Some(name) = &child.name {
                cryptsetup(&["close", name])?;
            }
        }
    }
    Ok(())
}

/// The whole data disk a format names, read again: internal, never the system, its identity
/// unchanged, and no volume group or array, which a format would break apart elsewhere.
pub(in crate::services::storage) fn format_target(request: &Request) -> Result<Device, String> {
    let Request::Format {
        device, identity, ..
    } = request
    else {
        return Err("not a format".to_owned());
    };
    let devices = inventory::list()?;
    let disk = devices
        .blockdevices
        .iter()
        .find(|disk| disk.name.as_deref() == Some(device))
        .ok_or("the disk is no longer connected")?;
    formattable(disk, device, identity, &inventory::system_uuids())?;
    Ok(disk.clone())
}

pub(super) fn formattable(
    disk: &Device,
    name: &str,
    identity: &str,
    system: &HashSet<String>,
) -> Result<(), String> {
    fn grouped(device: &Device) -> bool {
        device.kind == "lvm"
            || device.kind.starts_with("raid")
            || matches!(
                device.fstype.as_deref(),
                Some("LVM2_member" | "linux_raid_member")
            )
            || device.children.iter().flatten().any(grouped)
    }
    if disk.kind != "disk" || disk.path.as_deref() != Some(format!("/dev/{name}").as_str()) {
        return Err("only a whole disk can be formatted".to_owned());
    }
    if inventory::identity(disk) != identity {
        return Err("the disk is not the one that was selected: list the disks again".to_owned());
    }
    if disk.ro.unwrap_or(false) || inventory::external(disk) {
        return Err("only an internal writable disk can be formatted".to_owned());
    }
    if contains_system(disk, system) {
        return Err("the disk holds the system".to_owned());
    }
    if grouped(disk) {
        return Err("the disk is part of a volume group or an array".to_owned());
    }
    Ok(())
}

async fn format(request: &Request) -> Result<Answer, String> {
    let Request::Format {
        device,
        label,
        encrypt,
        automatic,
        password,
        ..
    } = request
    else {
        return Err("not a format".to_owned());
    };
    let disk = format_target(request)?;
    let log = log_path();
    if let Some(parent) = log.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(
        &log,
        format!("Disk format\nDevice: /dev/{device}\nEncrypted: {encrypt}\n\n"),
    )
    .map_err(|error| error.to_string())?;
    release(&disk)?;
    for container in inventory::containers(std::slice::from_ref(&disk)) {
        if let Some(uuid) = &container.uuid {
            let _ = fs::remove_file(credential(uuid));
            set_held(uuid, false)?;
        }
    }
    for child in disk.children.iter().flatten() {
        if let Some(path) = &child.path {
            output_logged("wipefs", &["--all", path], &log)?;
        }
    }
    output_logged("wipefs", &["--all", &format!("/dev/{device}")], &log)?;
    let partition = partition(device, Some(&log)).await?;
    let target = if *encrypt {
        encrypted(&partition, *automatic, password.as_deref())?
    } else {
        partition
    };
    output_logged("mkfs.ext4", &["-F", "-L", label, &target], &log)?;
    write_status();
    reconcile().await?;
    Ok(Answer::default())
}

/// Makes the partition a LUKS2 container with the password and the automatic key asked for, opens
/// it and returns its device.
fn encrypted(partition: &str, automatic: bool, password: Option<&str>) -> Result<String, String> {
    let password = password.map(|password| Secret::write("password", password.as_bytes()));
    let password = password.transpose()?;
    let key = if automatic {
        Some(Secret::write("automatic", &random(64)?)?)
    } else {
        None
    };
    let (first, slot, fast) = match (&password, &key) {
        (Some(password), _) => (password, PASSWORD_SLOT, false),
        (None, Some(key)) => (key, AUTOMATIC_SLOT, true),
        (None, None) => return Err("an encrypted volume needs a key".to_owned()),
    };
    let slot = slot.to_string();
    let mut arguments = vec![
        "luksFormat",
        "--type",
        "luks2",
        "--batch-mode",
        "--key-file",
        first.arg(),
        "--key-slot",
        &slot,
    ];
    if fast {
        arguments.extend(FAST);
    }
    arguments.push(partition);
    cryptsetup(&arguments)?;
    if let (Some(password), Some(key)) = (&password, &key) {
        add(partition, password, key, AUTOMATIC_SLOT, true)?;
    }
    let uuid = cryptsetup(&["luksUUID", partition])?;
    if !super::valid_uuid(&uuid) {
        return Err("the new container has no UUID".to_owned());
    }
    if let Some(key) = &key {
        save_automatic(&uuid, key)?;
    }
    let name = mapper(&uuid);
    cryptsetup(&["open", "--key-file", first.arg(), partition, &name])?;
    Ok(format!("/dev/mapper/{name}"))
}

async fn unlock(uuid: &str, password: &str) -> Result<Answer, String> {
    let (device, path) = container(uuid)?;
    if device.children.is_none_or(|children| children.is_empty()) {
        let key = Secret::write("unlock", password.as_bytes())?;
        if !opens(&path, &key, None) {
            return Err("the password does not open the volume".to_owned());
        }
        cryptsetup(&["open", "--key-file", key.arg(), &path, &mapper(uuid)])?;
    }
    set_held(uuid, false)?;
    write_status();
    reconcile().await?;
    Ok(Answer::default())
}

async fn lock(uuid: &str) -> Result<Answer, String> {
    let (device, _) = container(uuid)?;
    // Held first: a reconciliation in between must not open it again.
    set_held(uuid, true)?;
    release(&device)?;
    write_status();
    reconcile().await?;
    Ok(Answer::default())
}

fn automatic(uuid: &str, enabled: bool, password: Option<&String>) -> Result<Answer, String> {
    let (_, path) = container(uuid)?;
    if enabled {
        let current = current_key(uuid, &path, password)?;
        let key = Secret::write("automatic-new", &random(64)?)?;
        replace(&path, AUTOMATIC_SLOT, &current, &key, true)?;
        save_automatic(uuid, &key)?;
    } else {
        let present = slots(&path)?;
        if !present.iter().any(|slot| *slot != AUTOMATIC_SLOT) {
            return Err(
                "the volume would have no key left: set a password or a recovery key first"
                    .to_owned(),
            );
        }
        if present.contains(&AUTOMATIC_SLOT) {
            kill(&path, AUTOMATIC_SLOT)?;
        }
        let _ = fs::remove_file(credential(uuid));
    }
    write_status();
    Ok(Answer::default())
}

fn password(uuid: &str, current: Option<&String>, new: &str) -> Result<Answer, String> {
    let (_, path) = container(uuid)?;
    let current = current_key(uuid, &path, current)?;
    let new = Secret::write("password", new.as_bytes())?;
    replace(&path, PASSWORD_SLOT, &current, &new, false)?;
    write_status();
    Ok(Answer::default())
}

fn recovery(uuid: &str, current: Option<&String>) -> Result<Answer, String> {
    let (_, path) = container(uuid)?;
    let current = current_key(uuid, &path, current)?;
    let text = recovery_key()?;
    let new = Secret::write("recovery", text.as_bytes())?;
    replace(&path, RECOVERY_SLOT, &current, &new, true)?;
    write_status();
    Ok(Answer {
        recovery: Some(text),
        ..Answer::default()
    })
}

fn header(uuid: &str) -> Result<Answer, String> {
    let (_, path) = container(uuid)?;
    let backup = Secret::adopt("header")?;
    cryptsetup(&[
        "luksHeaderBackup",
        "--header-backup-file",
        backup.arg(),
        &path,
    ])?;
    let bytes = fs::read(&backup.0).map_err(|error| error.to_string())?;
    Ok(Answer {
        header: Some(base64(&bytes)),
        ..Answer::default()
    })
}

async fn run(request: &Request) -> Result<Answer, String> {
    validate(request)?;
    match request {
        Request::Format { .. } => format(request).await,
        Request::Unlock { uuid, password } => unlock(uuid, password).await,
        Request::Lock { uuid } => lock(uuid).await,
        Request::Automatic {
            uuid,
            enabled,
            password,
        } => automatic(uuid, *enabled, password.as_ref()),
        Request::Password {
            uuid,
            current,
            password: new,
        } => password(uuid, current.as_ref(), new),
        Request::Recovery { uuid, current } => recovery(uuid, current.as_ref()),
        Request::Header { uuid } => header(uuid),
    }
}

/// What the socket carries: encryption, or the network disks.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Operation {
    Crypt(Request),
    Network(NetworkRequest),
}

/// One request on standard input, one answer on standard output.
pub async fn serve() -> Result<(), String> {
    let mut line = String::new();
    std::io::stdin()
        .take(REQUEST_LIMIT)
        .read_to_string(&mut line)
        .map_err(|error| error.to_string())?;
    let result = match serde_json::from_str::<Operation>(line.trim()) {
        Ok(Operation::Crypt(request)) => run(&request).await,
        Ok(Operation::Network(request)) => network::root::run(&request).await,
        Err(_) => Err("invalid request".to_owned()),
    };
    let answer = result.unwrap_or_else(|error| Answer {
        error: Some(error),
        ..Answer::default()
    });
    let encoded = serde_json::to_vec(&answer).map_err(|error| error.to_string())?;
    std::io::stdout()
        .write_all(&encoded)
        .map_err(|error| error.to_string())
}

/// What the reconciliation does for encryption, as root: it opens with their automatic key the
/// closed containers the person did not close, and says whether it opened one.
pub(in crate::services::storage) fn open_automatic(devices: &[Device]) -> bool {
    let closed = held();
    let system = inventory::system_uuids();
    let crypttab = crypttab_uuids();
    let mut opened = false;
    for device in inventory::containers(devices) {
        let (Some(uuid), Some(path)) = (&device.uuid, &device.path) else {
            continue;
        };
        if device
            .children
            .as_ref()
            .is_some_and(|children| !children.is_empty())
            || closed.contains(&uuid.to_ascii_lowercase())
            || !credential(uuid).is_file()
            || system_container(device, &system, &crypttab)
        {
            continue;
        }
        match load_automatic(uuid).and_then(|key| {
            output(
                "cryptsetup",
                &["open", "--key-file", key.arg(), path, &mapper(uuid)],
            )
        }) {
            Ok(_) => opened = true,
            Err(reason) => eprintln!("{uuid} not opened: {reason}"),
        }
    }
    opened
}

fn write_status() {
    if let Ok(devices) = inventory::list() {
        write_status_of(&devices.blockdevices);
    }
}

/// Writes for the launcher which keys each container has: the slots of LUKS, the credential of
/// the automatic key and whether the person closed it.
pub(in crate::services::storage) fn write_status_of(devices: &[Device]) {
    let closed = held();
    let mut status = HashMap::new();
    for device in inventory::containers(devices) {
        let (Some(uuid), Some(path)) = (&device.uuid, &device.path) else {
            continue;
        };
        let uuid = uuid.to_ascii_lowercase();
        status.insert(
            uuid.clone(),
            keys_of(slots(path).ok().as_deref(), &uuid, &closed),
        );
    }
    if let Ok(text) = serde_json::to_vec(&status) {
        let _ = fs::write(status_path(), text);
    }
}

/// The keys of a container from its slots: unknown slots (LUKS1, or a container made elsewhere)
/// count as its password.
pub(super) fn keys_of(slots: Option<&[u8]>, uuid: &str, closed: &HashSet<String>) -> Keys {
    let slots = slots.unwrap_or(&[PASSWORD_SLOT]);
    Keys {
        password: slots
            .iter()
            .any(|slot| *slot != AUTOMATIC_SLOT && *slot != RECOVERY_SLOT),
        automatic: slots.contains(&AUTOMATIC_SLOT) && credential(uuid).is_file(),
        recovery: slots.contains(&RECOVERY_SLOT),
        held: closed.contains(uuid),
    }
}

#[cfg(test)]
#[path = "../../../../../tests/services/storage/crypt/root.rs"]
mod tests;
