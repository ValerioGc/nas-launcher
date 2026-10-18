//! Validated system cleanup operations exposed through `permissions.cleanup`.
//!
//! Requests select a fixed operation and numeric settings. The privileged executor
//! uses predefined paths and commands in `nas-os-cleanup.service`. Preview requests
//! measure usage without deleting files. Application caches use `services::caches`.
//! Data disks, home directories, application state, quarantine, and backup rules
//! are excluded from system cleanup.

pub mod executor;

use crate::{
    http::error::ApiFailure,
    services::database,
    utils::{self, Mode},
};
use serde::{Deserialize, Serialize};
use std::{sync::LazyLock, time::Duration};
use tokio::sync::Mutex;

const UNIT: &str = "nas-os-cleanup.service";
/// Maximum cleanup duration, including time spent waiting for apt.
const LONGEST: Duration = Duration::from_secs(2 * 60 * 60);
/// Serializes cleanup jobs sharing apt and the request file.
static LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    /// The disk of the system by category, and the data disks.
    Overview,
    /// Estimates reclaimable space without removing files.
    Preview,
    /// Frees it, and says how much.
    Clean,
}

/// Allowed system cleanup operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Item {
    /// The packages apt downloaded (`apt-get clean`).
    AptArchives,
    /// The downloaded packages no longer offered (`apt-get autoclean`).
    AptObsolete,
    /// The packages installed only for others no longer there (`apt-get autoremove`).
    AptOrphans,
    /// The configuration left by packages removed (`dpkg --purge`).
    AptConfigs,
    /// The kernels older than the one running and the one before it.
    OldKernels,
    /// The logs rotated in `/var/log`.
    RotatedLogs,
    /// The journal, down to the size of the settings.
    Journal,
    /// The files of `/tmp` and `/var/tmp` older than the days of the settings.
    Temporary,
    /// The backups of the updates of the NAS but the last one.
    NasBackups,
    /// Downloaded releases and application versions left by interrupted updates.
    NasLeftovers,
}

impl Item {
    pub const ALL: [Self; 10] = [
        Self::AptArchives,
        Self::AptObsolete,
        Self::AptOrphans,
        Self::AptConfigs,
        Self::OldKernels,
        Self::RotatedLogs,
        Self::Journal,
        Self::Temporary,
        Self::NasBackups,
        Self::NasLeftovers,
    ];
}

/// The two numbers of the settings of the Cleanup app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    /// The age, in days, past which a temporary file goes.
    pub temporary_days: u16,
    /// The size, in MiB, the journal is brought down to.
    pub journal_mb: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            temporary_days: 7,
            journal_mb: 500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub action: Action,
    #[serde(default)]
    pub item: Option<Item>,
    #[serde(default)]
    pub options: Options,
}

impl Request {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=3650).contains(&self.options.temporary_days) {
            return Err("the age of the temporary files is 1 to 3650 days".into());
        }
        if !(32..=102_400).contains(&self.options.journal_mb) {
            return Err("the size of the journal is 32 to 102400 MiB".into());
        }
        match (self.action, self.item) {
            (Action::Overview, None) | (Action::Preview | Action::Clean, Some(_)) => Ok(()),
            (Action::Overview, Some(_)) => Err("the overview takes no item".into()),
            _ => Err("a preview or a cleanup names its item".into()),
        }
    }
}

/// Measured item state before or after cleanup.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub item: Option<Item>,
    pub bytes: u64,
    /// Files or packages, the first ones.
    pub entries: Vec<String>,
    /// Number of additional entries omitted from the detailed list.
    pub more: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Disk {
    pub name: String,
    pub total: u64,
    pub free: u64,
}

/// The disk of the system, with what takes room on it, and the data disks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub total: u64,
    pub free: u64,
    pub categories: Vec<Category>,
    pub disks: Vec<Disk>,
}

/// Cleanup result or failure written by the privileged helper.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Runs a request in `nas-os-cleanup.service` and returns its answer.
pub async fn run(request: Request) -> Result<serde_json::Value, ApiFailure> {
    request.validate().map_err(ApiFailure::bad)?;
    if !utils::is_real(Mode::Apps) {
        return Err(ApiFailure::unavailable(
            "The cleanup is not available on this installation.",
        ));
    }
    let _alone = LOCK.lock().await;
    let (Some(asked), Some(answered)) = (
        database::beside("cleanup.request.json"),
        database::beside("cleanup.result.json"),
    ) else {
        return Err(ApiFailure::unavailable("no state directory"));
    };
    let _ = tokio::fs::remove_file(&answered).await;
    let bytes = serde_json::to_vec(&request)
        .map_err(|_| ApiFailure::unavailable("cannot encode the cleanup"))?;
    tokio::fs::write(&asked, bytes)
        .await
        .map_err(|_| ApiFailure::unavailable("cannot write the cleanup request"))?;
    let started = utils::systemctl(&["start", UNIT], LONGEST).await;
    let _ = tokio::fs::remove_file(&asked).await;
    let answer = tokio::fs::read(&answered)
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Answer>(&bytes).ok());
    let _ = tokio::fs::remove_file(&answered).await;
    outcome(answer, started)
}

/// Reads the helper result, preferring its error over systemd diagnostics.
pub fn outcome(
    answer: Option<Answer>,
    started: Result<(), String>,
) -> Result<serde_json::Value, ApiFailure> {
    match (answer, started) {
        (
            Some(Answer {
                value: Some(value), ..
            }),
            _,
        ) => Ok(value),
        (
            Some(Answer {
                error: Some(error), ..
            }),
            _,
        ) => Err(ApiFailure::unavailable(error)),
        (_, Err(reason)) => Err(ApiFailure::unavailable(reason)),
        _ => Err(ApiFailure::unavailable("the cleanup gave no answer")),
    }
}

#[cfg(test)]
#[path = "../../tests/services/cleanup.rs"]
mod tests;
