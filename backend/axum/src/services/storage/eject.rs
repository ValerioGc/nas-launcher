//! Validated external-disk ejection and persistent removal locks.
//!
//! The launcher rejects locked disks and submits requests to a privileged helper.
//! The helper revalidates disk identity, unmounts Explorer views before raw mounts,
//! detaches the device, and republishes the remaining volumes. System disks are
//! excluded. Locks are stored by filesystem UUID and survive device-name changes.

use super::{
    contains_system,
    inventory::{self, external},
    output_logged, reconcile, system_mount, valid_device_name, Device, Devices, EXPLORER_ROOT,
    VOLUMES_ROOT,
};
use crate::{
    http::error::ApiFailure,
    services::notifications::{self, Notice, Tone},
    utils::{self, now_ms, Mode},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

const EJECT_UNIT: &str = "nas-os-storage-eject.service";

#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EjectRequest {
    /// Whole disk or partition identifying the disk to eject.
    pub device: String,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LockRequest {
    /// The UUID of the filesystem of the volume.
    pub uuid: String,
    /// True marks the disk as not removable.
    pub locked: bool,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExternalDisk {
    pub device: String,
    pub name: String,
    pub locked: bool,
    pub can_eject: bool,
}

fn external_inventory(
    devices: Devices,
    locked: &[String],
    system_uuids: &HashSet<String>,
) -> Vec<ExternalDisk> {
    devices
        .blockdevices
        .into_iter()
        .filter(|disk| disk.kind == "disk" && external(disk))
        .filter_map(|disk| {
            let device = disk.name.clone().filter(|name| valid_device_name(name))?;
            let name = disk
                .label
                .clone()
                .filter(|label| !label.trim().is_empty())
                .or_else(|| {
                    disk.children.iter().flatten().find_map(|child| {
                        child.label.clone().filter(|label| !label.trim().is_empty())
                    })
                })
                .unwrap_or_else(|| device.clone());
            let marked = uuids(&disk).iter().any(|uuid| locked.contains(uuid));
            Some(ExternalDisk {
                device,
                name,
                locked: marked,
                can_eject: !marked && ejectable(&disk, system_uuids).is_ok(),
            })
        })
        .collect()
}

/// Connected external devices for the header, including unmounted and unformatted disks.
pub async fn external_disks(db: &SqlitePool) -> Result<Vec<ExternalDisk>, ApiFailure> {
    let locked = locks(db).await?;
    tokio::task::spawn_blocking(move || {
        let devices = inventory::list().map_err(ApiFailure::unavailable)?;
        Ok(external_inventory(
            devices,
            &locked,
            &inventory::system_uuids(),
        ))
    })
    .await
    .map_err(|_| ApiFailure::unavailable("disk inspection failed"))?
}

fn request_path() -> PathBuf {
    super::state_file("storage-eject.json")
}

fn error_path() -> PathBuf {
    super::state_file("storage-eject.error")
}

fn log_path() -> PathBuf {
    std::env::var_os("NAS_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/log/nas-os"))
        .join("eject-storage.log")
}

fn contains(block: &Device, name: &str) -> bool {
    block.name.as_deref() == Some(name)
        || block
            .children
            .iter()
            .flatten()
            .any(|child| contains(child, name))
}

/// The whole disk that holds `device` in a tree of `lsblk`.
pub(super) fn disk_of<'a>(devices: &'a [Device], device: &str) -> Option<&'a Device> {
    devices.iter().find(|disk| contains(disk, device))
}

/// The UUIDs of the filesystems on a disk, in lower case.
pub(super) fn uuids(block: &Device) -> Vec<String> {
    let mut found: Vec<String> = block
        .uuid
        .iter()
        .filter(|uuid| !uuid.is_empty())
        .map(|uuid| uuid.to_ascii_lowercase())
        .collect();
    for child in block.children.iter().flatten() {
        found.extend(uuids(child));
    }
    found
}

/// Reports whether a disk is non-removable or contains system filesystems.
pub(super) fn ejectable(disk: &Device, system_uuids: &HashSet<String>) -> Result<(), String> {
    if disk.kind != "disk" || !external(disk) {
        return Err("only an external disk can be ejected".to_owned());
    }
    if contains_system(disk, system_uuids) {
        return Err("the disk holds a filesystem of the system".to_owned());
    }
    Ok(())
}

/// Orders unmounts with Explorer bind mounts first, raw mounts next, and other mounts last.
pub(super) fn mountpoints(block: &Device) -> Vec<String> {
    fn gather(block: &Device, found: &mut Vec<String>) {
        found.extend(block.mountpoints.iter().flatten().flatten().cloned());
        for child in block.children.iter().flatten() {
            gather(child, found);
        }
    }
    let mut found = Vec::new();
    gather(block, &mut found);
    found.retain(|path| path.starts_with('/'));
    found.sort_by_key(|path| {
        if path.starts_with(EXPLORER_ROOT) {
            0
        } else if path.starts_with(VOLUMES_ROOT) {
            1
        } else {
            2
        }
    });
    found.dedup();
    found
}

/// The disk of the request, read again, and what is on it.
fn inspect(device: &str, system_uuids: &HashSet<String>) -> Result<Device, String> {
    if !valid_device_name(device) {
        return Err("invalid device".to_owned());
    }
    let devices: Devices = inventory::list()?;
    let disk = disk_of(&devices.blockdevices, device)
        .ok_or("the disk is no longer connected")?
        .clone();
    ejectable(&disk, system_uuids)?;
    Ok(disk)
}

/// The UUIDs marked as not removable.
pub async fn locks(db: &SqlitePool) -> Result<Vec<String>, ApiFailure> {
    sqlx::query_scalar("SELECT uuid FROM storage_locks ORDER BY uuid")
        .fetch_all(db)
        .await
        .map_err(|_| ApiFailure::unavailable("the marks of the disks cannot be read"))
}

/// Marks the disk of a filesystem as not removable, or removable again.
pub async fn set_lock(db: &SqlitePool, input: LockRequest) -> Result<Vec<String>, ApiFailure> {
    let uuid = input.uuid.to_ascii_lowercase();
    if !(1..=64).contains(&uuid.len())
        || !uuid
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(ApiFailure::bad("invalid filesystem UUID"));
    }
    let query = if input.locked {
        sqlx::query(
            "INSERT INTO storage_locks (uuid, created_at) VALUES (?, ?) ON CONFLICT DO NOTHING",
        )
        .bind(uuid)
        .bind(now_ms())
    } else {
        sqlx::query("DELETE FROM storage_locks WHERE uuid = ?").bind(uuid)
    };
    query
        .execute(db)
        .await
        .map_err(|_| ApiFailure::unavailable("the mark of the disk was not saved"))?;
    locks(db).await
}

/// Checks the request, refuses a disk marked as not removable, and has the root unit eject it.
pub async fn eject(db: &SqlitePool, input: EjectRequest) -> Result<(), ApiFailure> {
    if !valid_device_name(&input.device) {
        return Err(ApiFailure::bad("invalid device"));
    }
    if !utils::is_real(Mode::Storage) {
        return Err(ApiFailure::unavailable(
            "Disk ejection is not available on this installation.",
        ));
    }
    let device = input.device.clone();
    let disk = tokio::task::spawn_blocking(move || inspect(&device, &inventory::system_uuids()))
        .await
        .map_err(|_| ApiFailure::unavailable("disk inspection failed"))?
        .map_err(ApiFailure::bad)?;
    let locked = locks(db).await?;
    if uuids(&disk).iter().any(|uuid| locked.contains(uuid)) {
        return Err(ApiFailure::forbidden("the disk is marked as not removable"));
    }

    let path = request_path();
    let temporary = path.with_extension("json.new");
    let encoded = serde_json::to_vec(&input)
        .map_err(|_| ApiFailure::unavailable("cannot prepare the ejection"))?;
    tokio::fs::write(&temporary, encoded)
        .await
        .map_err(|_| ApiFailure::unavailable("cannot save the ejection request"))?;
    tokio::fs::rename(&temporary, &path)
        .await
        .map_err(|_| ApiFailure::unavailable("cannot save the ejection request"))?;
    let failure = error_path();
    let _ = tokio::fs::remove_file(&failure).await;
    let outcome = utils::systemctl(&["start", EJECT_UNIT], Duration::from_secs(120)).await;
    let _ = tokio::fs::remove_file(path).await;
    let reason = match outcome {
        Ok(()) => {
            let _ = tokio::fs::remove_file(failure).await;
            return Ok(());
        }
        Err(fallback) => tokio::fs::read_to_string(&failure)
            .await
            .ok()
            .filter(|reason| !reason.trim().is_empty())
            .unwrap_or(fallback),
    };
    let _ = tokio::fs::remove_file(failure).await;
    notifications::record(
        db,
        "nas-disk-manager",
        Tone::Error,
        Notice::new("diskEjectFailed", "Disk not ejected", reason.clone())
            .with("disk", input.device)
            .with("reason", reason.clone())
            .log(log_path().display().to_string()),
    )
    .await;
    Err(ApiFailure::unavailable(reason))
}

/// Unmounts every volume of the disk and detaches it from the kernel. A disk with no `delete` in
/// sysfs (some card readers) stays attached, unmounted, which is as safe to unplug.
fn detach(disk: &Device, log: &Path) -> Result<(), String> {
    output_logged("sync", &[], log)?;
    for mountpoint in mountpoints(disk) {
        if system_mount(&mountpoint) {
            return Err(format!("{mountpoint} belongs to the system"));
        }
        output_logged("umount", &[&mountpoint], log)?;
    }
    let name = disk.name.as_deref().ok_or("the disk has no name")?;
    let delete = Path::new("/sys/block").join(name).join("device/delete");
    let note = if delete.exists() {
        fs::write(&delete, "1").map_err(|error| format!("{}: {error}", delete.display()))?;
        format!("/dev/{name} detached: it can be unplugged\n")
    } else {
        format!("/dev/{name} unmounted, with no way to detach it: it can be unplugged\n")
    };
    let _ = fs::OpenOptions::new()
        .append(true)
        .open(log)
        .and_then(|mut file| std::io::Write::write_all(&mut file, note.as_bytes()));
    Ok(())
}

/// Revalidates the request as root, unmounts and detaches the disk, then refreshes volume
/// state.
pub async fn eject_reported(path: &Path) -> Result<(), String> {
    let result = run_reported(path).await;
    if let Err(reason) = &result {
        let _ = fs::write(error_path(), reason);
    }
    result
}

async fn run_reported(path: &Path) -> Result<(), String> {
    let input: EjectRequest = serde_json::from_slice(
        &fs::read(path).map_err(|error| format!("cannot read the ejection request: {error}"))?,
    )
    .map_err(|error| format!("invalid ejection request: {error}"))?;
    let fstab = fs::read_to_string("/etc/fstab").map_err(|error| error.to_string())?;
    let disk = inspect(&input.device, &inventory::fstab_system_uuids(&fstab))?;
    let log = log_path();
    if let Some(parent) = log.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(
        &log,
        format!("Disk ejection\nDevice: /dev/{}\n\n", input.device),
    )
    .map_err(|error| error.to_string())?;
    detach(&disk, &log)?;
    reconcile().await
}

#[cfg(test)]
#[path = "../../../tests/services/storage/eject.rs"]
mod tests;
