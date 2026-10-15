//! Encrypted data volumes, LUKS2: formatting a data disk with or without encryption, opening and
//! closing a container, its automatic key, its password, its recovery key and the backup of its
//! header.
//!
//! The launcher never touches a device. It sends one request, secrets included, on the connection
//! of `nas-os-storage-crypt.socket`, which only the group `nas-os-crypt` may open (the launcher is
//! its one member: the applications share the group `nas-os`, not this one), and
//! `nas-os-storage-crypt@.service` runs `nas-os-axum --storage-crypt` as root on that connection
//! ([`root`]). A secret stays in memory and, for the time of one command, in a file of the runtime
//! directory of the unit (a tmpfs only root reads): never in an argument, a log or a task.
//!
//! The key slots: 0 is the password of the person, 1 the automatic key, 2 the recovery key. The
//! automatic key is random and kept as the encrypted credential `nas-os.luks-<uuid>` of systemd
//! (the key of the machine, and the TPM where there is one): the reconciliation opens the container
//! with it after the boot, unless the person closed the volume, which stays closed until opened
//! again. Everything needs `NAS_ENCRYPTION_MODE=real`, off by default until it is tried on a machine.

pub mod root;

use super::{inventory, valid_device_name};
use crate::{
    http::error::ApiFailure,
    services::notifications::{self, Notice, Tone},
    utils::{self, Mode},
    widgets::services::tasks::{self, NewTask, TaskState, TaskUpdate},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{collections::HashMap, path::PathBuf, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) const PASSWORD_SLOT: u8 = 0;
pub(super) const AUTOMATIC_SLOT: u8 = 1;
pub(super) const RECOVERY_SLOT: u8 = 2;
const FORMAT_TASK: &str = "format-storage";
/// A header of LUKS2 is 16 MiB, a little more in base64.
const ANSWER_LIMIT: u64 = 32 * 1024 * 1024;
const MIN_PASSWORD: usize = 8;
const MAX_PASSWORD: usize = 512;

/// What the launcher asks of the root unit.
#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum Request {
    /// Erases a data disk and makes one volume on it, encrypted or not. An encrypted one opens
    /// with the password, the automatic key, or both.
    #[serde(rename_all = "camelCase")]
    Format {
        device: String,
        identity: String,
        label: String,
        encrypt: bool,
        #[serde(default)]
        automatic: bool,
        #[serde(default)]
        password: Option<String>,
    },
    /// Opens a closed container with the password or the recovery key, and publishes it.
    Unlock { uuid: String, password: String },
    /// Takes the volume off Explorer, unmounts it and closes the container, which stays closed.
    Lock { uuid: String },
    /// Adds or removes the automatic key. Adding it needs a key that opens the container.
    #[serde(rename_all = "camelCase")]
    Automatic {
        uuid: String,
        enabled: bool,
        #[serde(default)]
        password: Option<String>,
    },
    /// Sets or replaces the password, with a key that opens the container, or with the automatic
    /// key when `current` is missing.
    Password {
        uuid: String,
        #[serde(default)]
        current: Option<String>,
        password: String,
    },
    /// Makes a new recovery key, which replaces the one before.
    Recovery {
        uuid: String,
        #[serde(default)]
        current: Option<String>,
    },
    /// The backup of the header of the container.
    Header { uuid: String },
}

/// What the root unit answers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// The new recovery key, shown once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<String>,
    /// The backup of the header, in base64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    /// The task of a format, which goes on in the background.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Why the root unit refused or failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The keys a container has and whether the person closed it, as the root unit last saw them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, utoipa::ToSchema)]
pub struct Keys {
    pub password: bool,
    pub automatic: bool,
    pub recovery: bool,
    /// Closed by the person: nothing opens it but the person.
    pub held: bool,
}

pub(super) fn valid_uuid(uuid: &str) -> bool {
    (1..=64).contains(&uuid.len())
        && uuid
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// A secret goes to cryptsetup as the bytes of a file: no line break, no NUL, no control.
pub(super) fn valid_secret(secret: &str) -> bool {
    (MIN_PASSWORD..=MAX_PASSWORD).contains(&secret.chars().count())
        && !secret.chars().any(char::is_control)
}

pub(super) fn valid_label(label: &str) -> bool {
    label.trim() == label
        && (1..=16).contains(&label.len())
        && label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'-' | b'_'))
}

fn secret(value: Option<&String>, what: &str) -> Result<(), String> {
    match value {
        Some(value) if !valid_secret(value) => Err(format!(
            "the {what} needs {MIN_PASSWORD} to {MAX_PASSWORD} characters, no control character"
        )),
        _ => Ok(()),
    }
}

/// The request checked on its own, before anything looks at the machine.
pub fn validate(request: &Request) -> Result<(), String> {
    let uuid = match request {
        Request::Format {
            device,
            identity,
            label,
            encrypt,
            automatic,
            password,
        } => {
            return validate_format(
                device,
                identity,
                label,
                *encrypt,
                *automatic,
                password.as_ref(),
            );
        }
        Request::Unlock { uuid, password } => {
            secret(Some(password), "password")?;
            uuid
        }
        Request::Automatic {
            uuid,
            enabled,
            password,
        } => {
            if *enabled && password.is_none() {
                return Err("the automatic key needs a key that opens the volume".to_owned());
            }
            secret(password.as_ref(), "password")?;
            uuid
        }
        Request::Password {
            uuid,
            current,
            password,
        } => {
            secret(current.as_ref(), "current password")?;
            secret(Some(password), "password")?;
            uuid
        }
        Request::Recovery { uuid, current } => {
            secret(current.as_ref(), "current password")?;
            uuid
        }
        Request::Lock { uuid } | Request::Header { uuid } => uuid,
    };
    if valid_uuid(uuid) {
        Ok(())
    } else {
        Err("invalid volume UUID".to_owned())
    }
}

fn validate_format(
    device: &str,
    identity: &str,
    label: &str,
    encrypt: bool,
    automatic: bool,
    password: Option<&String>,
) -> Result<(), String> {
    if !valid_device_name(device) {
        return Err("invalid disk device".to_owned());
    }
    if !inventory::valid_identity(identity) {
        return Err("invalid disk identity".to_owned());
    }
    if !valid_label(label) {
        return Err("invalid filesystem label".to_owned());
    }
    if encrypt && password.is_none() && !automatic {
        return Err("an encrypted volume needs a password or the automatic key".to_owned());
    }
    if !encrypt && (password.is_some() || automatic) {
        return Err("a volume without encryption has no keys".to_owned());
    }
    secret(password, "password")
}

fn socket_path() -> PathBuf {
    std::env::var_os("NAS_STORAGE_CRYPT_SOCKET").map_or_else(
        || PathBuf::from("/run/nas-os-storage-crypt.sock"),
        PathBuf::from,
    )
}

/// How long the root unit may take: a format partitions and makes a filesystem, a lock waits for
/// the unmounts, a header is 16 MiB.
fn time_for(request: &Request) -> Duration {
    Duration::from_secs(match request {
        Request::Format { .. } => 15 * 60,
        Request::Lock { .. } => 150,
        _ => 90,
    })
}

/// Sends a request on the socket of the root unit and reads its answer.
pub(super) async fn send<T: Serialize>(request: &T, timeout: Duration) -> Result<Answer, String> {
    let exchange = async {
        let mut stream = tokio::net::UnixStream::connect(socket_path())
            .await
            .map_err(|error| format!("the encryption service is not reachable: {error}"))?;
        let mut line = serde_json::to_vec(request).map_err(|error| error.to_string())?;
        line.push(b'\n');
        stream
            .write_all(&line)
            .await
            .map_err(|error| error.to_string())?;
        stream.shutdown().await.map_err(|error| error.to_string())?;
        let mut answer = Vec::new();
        stream
            .take(ANSWER_LIMIT)
            .read_to_end(&mut answer)
            .await
            .map_err(|error| error.to_string())?;
        serde_json::from_slice::<Answer>(&answer)
            .map_err(|_| "the encryption service gave no answer".to_owned())
    };
    let answer = tokio::time::timeout(timeout, exchange)
        .await
        .map_err(|_| "the encryption service did not answer in time".to_owned())??;
    match answer.error {
        Some(error) => Err(error),
        None => Ok(answer),
    }
}

/// Whether encryption is on for this installation.
pub fn available() -> bool {
    utils::is_real(Mode::Encryption)
}

/// Checks the request and has the root unit run it: a format in the background, as a task, the
/// rest at once.
pub async fn request(db: &SqlitePool, request: Request) -> Result<Answer, ApiFailure> {
    validate(&request).map_err(ApiFailure::bad)?;
    if !available() {
        return Err(ApiFailure::unavailable(
            "Disk encryption is not available on this installation.",
        ));
    }
    let Request::Format { device, .. } = &request else {
        return send(&request, time_for(&request))
            .await
            .map_err(ApiFailure::unavailable);
    };
    if tasks::list(db)
        .await?
        .iter()
        .any(|task| task.id == FORMAT_TASK && task.state.active())
    {
        return Err(ApiFailure::bad("another disk is being formatted"));
    }
    let checked = request.clone();
    tokio::task::spawn_blocking(move || root::format_target(&checked).map(|_| ()))
        .await
        .map_err(|_| ApiFailure::unavailable("disk inspection failed"))?
        .map_err(ApiFailure::bad)?;
    let task = tasks::register(
        db,
        NewTask {
            id: FORMAT_TASK.to_owned(),
            source: "nas-disk-manager".to_owned(),
            name: format!("Format {device}"),
            detail: "erasing and formatting".to_owned(),
            progress: None,
            pausable: false,
            cancellable: false,
        },
    )
    .await?;
    let pool = db.clone();
    tokio::spawn(async move {
        let (state, detail, progress) = match send(&request, time_for(&request)).await {
            Ok(_) => (TaskState::Done, "ready for use".to_owned(), Some(100)),
            Err(reason) => (TaskState::Failed, reason, None),
        };
        let _ = tasks::update(
            &pool,
            FORMAT_TASK,
            TaskUpdate {
                state: Some(state),
                detail: Some(detail),
                progress: Some(progress),
                ..TaskUpdate::default()
            },
        )
        .await;
    });
    Ok(Answer {
        task_id: Some(task.id),
        ..Answer::default()
    })
}

pub(super) fn status_path() -> PathBuf {
    super::state_file("storage-crypt.json")
}

pub(super) fn held_path() -> PathBuf {
    super::state_file("storage-held")
}

/// The keys of each container, by UUID in lower case, as the root unit last wrote them.
pub(super) fn keys() -> HashMap<String, Keys> {
    std::fs::read(status_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Warns, while the warning before is unread, that an encrypted volume opens only with the
/// automatic key of this machine: the case of a disk the wizard encrypted, before the person set
/// a password or a recovery key.
pub async fn report_spare_keys(db: &SqlitePool) {
    let alone = keys()
        .values()
        .filter(|keys| keys.automatic && !keys.password && !keys.recovery)
        .count();
    if alone == 0 {
        return;
    }
    let unread: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM notifications WHERE key = 'diskWithoutSpareKey' AND read = 0)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(true);
    if !unread {
        notifications::record(
            db,
            "nas-disk-manager",
            Tone::Warning,
            Notice::new(
                "diskWithoutSpareKey",
                "Encrypted disk without a password",
                "An encrypted disk opens only on this NAS: set a password or a recovery key in Disks.",
            ),
        )
        .await;
    }
}

#[cfg(test)]
#[path = "../../../../tests/services/storage/crypt.rs"]
mod tests;
