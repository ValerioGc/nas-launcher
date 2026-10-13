//! The exact diagnostic detail behind a warning or error, for nas-log-manager.

use crate::{
    http::error::ApiFailure,
    services::notifications::{Notification, Tone},
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;

const LOG_ROOT: &str = "/var/log";
const TAIL_BYTES: u64 = 256 * 1024;
const MAX_LINES: usize = 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationLog {
    pub kind: &'static str,
    pub origin: String,
    pub lines: Vec<String>,
}

/// The log of a notification.
pub async fn read(notification: &Notification) -> Result<NotificationLog, ApiFailure> {
    if !matches!(notification.content.tone, Tone::Warning | Tone::Error) {
        return Err(ApiFailure::bad("this notification has no diagnostic log"));
    }
    match &notification.content.log {
        Some(path) => file(Path::new(LOG_ROOT), path).await,
        None => Err(ApiFailure::missing("diagnostic log")),
    }
}

pub async fn file(root: &Path, path: &str) -> Result<NotificationLog, ApiFailure> {
    let unreadable = |reason: String| ApiFailure::unavailable(format!("{path}: {reason}"));
    let resolved: PathBuf = tokio::fs::canonicalize(path)
        .await
        .map_err(|error| unreadable(error.to_string()))?;

    if !resolved.starts_with(root) {
        return Err(ApiFailure::bad(format!(
            "a log is read from {} only",
            root.display()
        )));
    }

    let mut opened = tokio::fs::File::open(&resolved)
        .await
        .map_err(|error| unreadable(error.to_string()))?;

    let size = opened
        .metadata()
        .await
        .map_err(|error| unreadable(error.to_string()))?
        .len();

    let skip = size.saturating_sub(TAIL_BYTES);
    let mut bytes = Vec::new();
    tokio::io::AsyncSeekExt::seek(&mut opened, std::io::SeekFrom::Start(skip))
        .await
        .map_err(|error| unreadable(error.to_string()))?;
    opened
        .take(TAIL_BYTES)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| unreadable(error.to_string()))?;

    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if skip > 0 && !lines.is_empty() {
        lines.remove(0);
    }

    Ok(NotificationLog {
        kind: "file",
        origin: resolved.display().to_string(),
        lines: last(lines),
    })
}

fn last(mut lines: Vec<String>) -> Vec<String> {
    let extra = lines.len().saturating_sub(MAX_LINES);
    lines.drain(..extra);
    lines
}

#[cfg(test)]
#[path = "../../../../../tests/apps/services/nas_notification_manager/log.rs"]
mod tests;
