//! Persistent notification log. [`record`] logs failures without failing the action
//! that produced the notification; [`create`] returns failures to its caller.

use crate::{http::error::ApiFailure, utils::now_ms};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{collections::BTreeMap, path::Path, path::PathBuf, time::Duration};

const LIMIT: i64 = 100;
/// The lines of the journal of a unit a diagnostic detail carries.
const JOURNAL_LINES: &str = "200";
const JOURNAL_TIMEOUT: Duration = Duration::from_secs(5);

/// Diagnostic log directory shared with the NAS log manager.
fn log_dir() -> PathBuf {
    std::env::var_os("NAS_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/log/nas-os"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Info,
    Success,
    Warning,
    Error,
}
impl Tone {
    pub fn as_str(self) -> &'static str {
        match self {
            Tone::Info => "info",
            Tone::Success => "success",
            Tone::Warning => "warning",
            Tone::Error => "error",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "info" => Some(Tone::Info),
            "success" => Some(Tone::Success),
            "warning" => Some(Tone::Warning),
            "error" => Some(Tone::Error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewNotification {
    pub title: String,
    pub message: String,
    pub source: String,
    pub tone: Tone,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub params: BTreeMap<String, String>,
    #[serde(default)]
    pub log: Option<String>,
}
impl NewNotification {
    pub fn validate(&self) -> Result<(), ApiFailure> {
        let identifier = |value: &str, max: usize| {
            !value.is_empty()
                && value.len() <= max
                && value
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        };
        let params = self.params.len() <= 12
            && self
                .params
                .iter()
                .all(|(name, value)| identifier(name, 32) && value.len() <= 500);
        let log = self.log.as_deref().is_none_or(|path| {
            path.starts_with('/')
                && path.len() <= 512
                && !path.chars().any(char::is_control)
                && !path.split('/').any(|part| part == "..")
        });
        if self.title.trim().is_empty()
            || self.title.len() > 160
            || self.message.len() > 2000
            || !identifier(&self.source, 64)
            || !self.key.as_deref().is_none_or(|key| identifier(key, 64))
            || !params
            || !log
        {
            return Err(ApiFailure::bad("invalid notification"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Notice {
    pub key: &'static str,
    pub title: String,
    pub message: String,
    pub params: BTreeMap<String, String>,
    pub log: Option<String>,
}
impl Notice {
    pub fn new(key: &'static str, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            key,
            title: title.into(),
            message: message.into(),
            params: BTreeMap::new(),
            log: None,
        }
    }

    pub fn with(mut self, name: &str, value: impl Into<String>) -> Self {
        self.params.insert(name.to_owned(), value.into());
        self
    }

    pub fn log(mut self, path: impl Into<String>) -> Self {
        self.log = Some(path.into());
        self
    }

    /// Writes failure details to a named log and links it from the notification.
    /// Includes recent unit journal output when available. If logging fails, the
    /// notification still retains its message.
    pub async fn detail(self, name: &str, reason: &str, unit: Option<&str>) -> Self {
        self.detail_in(&log_dir(), name, reason, unit).await
    }

    pub async fn detail_in(self, dir: &Path, name: &str, reason: &str, unit: Option<&str>) -> Self {
        let file: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let path = dir.join(format!("{file}.log"));
        let mut text = format!("{}\n{}\n\n{}\n", self.title, self.message, reason.trim());
        if let Some(unit) = unit {
            text.push_str(&format!("\nJournal of {unit}:\n{}\n", journal(unit).await));
        }
        if tokio::fs::create_dir_all(dir).await.is_err()
            || tokio::fs::write(&path, text).await.is_err()
        {
            return self;
        }
        self.log(path.display().to_string())
    }
}

/// The last lines of the journal of a unit, or why they cannot be read.
async fn journal(unit: &str) -> String {
    let output = tokio::process::Command::new("journalctl")
        .args([
            "--unit",
            unit,
            "--lines",
            JOURNAL_LINES,
            "--no-pager",
            "--output=short-iso",
            "--quiet",
        ])
        .output();
    match tokio::time::timeout(JOURNAL_TIMEOUT, output).await {
        Ok(Ok(output)) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_owned(),
        Ok(Ok(output)) => String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        Ok(Err(error)) => error.to_string(),
        Err(_) => "journalctl did not answer in time".to_owned(),
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Notification {
    pub id: String,
    #[serde(flatten)]
    pub content: NewNotification,
    pub at: i64,
    pub read: bool,
}

/// The shape every query selects (`id, source, title, message, tone, read, created_at, key,
/// params, log`) so that building a `Notification` out of a row is written once.
pub type Row = (
    i64,
    String,
    String,
    String,
    String,
    i64,
    i64,
    Option<String>,
    String,
    Option<String>,
);

pub fn row(entry: Row) -> Notification {
    let (id, source, title, message, tone, read, created_at, key, params, log) = entry;
    Notification {
        id: id.to_string(),
        content: NewNotification {
            title,
            message,
            source,
            tone: Tone::parse(&tone).unwrap_or(Tone::Info),
            key,
            params: serde_json::from_str(&params).unwrap_or_default(),
            log,
        },
        at: created_at,
        read: read != 0,
    }
}

/// Records a notification and returns it.
pub async fn create(db: &SqlitePool, input: NewNotification) -> Result<Notification, ApiFailure> {
    input.validate()?;
    let at = now_ms();
    let params = serde_json::to_string(&input.params).map_err(|_| unavailable())?;
    let id: (i64,) = sqlx::query_as(
        "INSERT INTO notifications (source, title, message, tone, read, created_at, key, params, log)
         VALUES (?, ?, ?, ?, 0, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(&input.source)
    .bind(&input.title)
    .bind(&input.message)
    .bind(input.tone.as_str())
    .bind(at)
    .bind(&input.key)
    .bind(params)
    .bind(&input.log)
    .fetch_one(db)
    .await
    .map_err(|_| unavailable())?;

    prune(db).await;
    Ok(Notification {
        id: id.0.to_string(),
        content: input,
        at,
        read: false,
    })
}

/// Creates a trace of an event.
pub async fn record(db: &SqlitePool, source: &str, tone: Tone, notice: Notice) {
    let input = NewNotification {
        title: notice.title,
        message: notice.message,
        source: source.to_owned(),
        tone,
        key: Some(notice.key.to_owned()),
        params: notice.params,
        log: notice.log,
    };
    if let Err(failure) = create(db, input).await {
        eprintln!(
            "notification not recorded ({source}/{}): {}",
            notice.key, failure.1
        );
    }
}

/// Keeps the table at `LIMIT` rows.
pub async fn prune(db: &SqlitePool) {
    let deleted = sqlx::query(
        "DELETE FROM notifications
         WHERE id <= (SELECT MIN(id) FROM (SELECT id FROM notifications ORDER BY id DESC LIMIT ?)) - 1",
    )
    .bind(LIMIT)
    .execute(db)
    .await;
    if let Err(error) = deleted {
        eprintln!("notification log not pruned: {error}");
    }
}

pub fn unavailable() -> ApiFailure {
    ApiFailure::unavailable("notification log unavailable")
}

#[cfg(test)]
#[path = "../../tests/services/notifications.rs"]
mod tests;
