//! Operation register, not a scheduler. Owners report progress and honour requested
//! pause/cancellation states; the launcher stores and displays those states.

use crate::{
    http::error::ApiFailure,
    services::notifications::{self, Notice, Tone},
    utils::now_ms,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::PathBuf;

/// Maximum finished operations. Running and paused operations do not count toward this limit.
const LIMIT: i64 = 30;

/// How long a finished operation stays on show before it is swept up: long enough to be read
/// after the fact, short enough not to become a second log.
const KEEP_FINISHED_MS: i64 = 24 * 60 * 60 * 1000;

fn operation_log(name: &str) -> String {
    std::env::var_os("NAS_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/log/nas-os"))
        .join(name)
        .display()
        .to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    Running,
    Paused,
    Done,
    Failed,
    Cancelled,
}
impl TaskState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "running" => Some(Self::Running),
            "paused" => Some(Self::Paused),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    /// Still going: the only state in which the user can ask for anything.
    pub fn active(self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }
}

/// One operation, as the widget lists it.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub source: String,
    pub name: String,
    pub detail: String,
    /// Per cent, or absent when the owner cannot tell.
    pub progress: Option<u8>,
    pub state: TaskState,
    pub pausable: bool,
    pub cancellable: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// What an application sends to register an operation.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NewTask {
    pub id: String,
    pub source: String,
    pub name: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub progress: Option<u8>,
    #[serde(default)]
    pub pausable: bool,
    #[serde(default)]
    pub cancellable: bool,
}

/// What the owner sends while it works. Every field is optional: reporting only the progress is
/// the common case, and leaving a field out does not clear it.
#[derive(Debug, Clone, Default, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TaskUpdate {
    pub name: Option<String>,
    pub detail: Option<String>,
    /// `Some(None)` (an explicit `null`) is how an owner says it no longer knows.
    #[serde(default, deserialize_with = "double_option")]
    pub progress: Option<Option<u8>>,
    pub state: Option<TaskState>,
}

/// Tells `null` apart from a missing field, which is what makes "clear the progress" possible.
fn double_option<'de, D>(deserializer: D) -> Result<Option<Option<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<u8>::deserialize(deserializer).map(Some)
}

type Row = (
    String,
    String,
    String,
    String,
    Option<i64>,
    String,
    i64,
    i64,
    i64,
    i64,
);

fn row(entry: Row) -> Task {
    let (id, source, name, detail, progress, state, pausable, cancellable, created_at, updated_at) =
        entry;
    Task {
        id,
        source,
        name,
        detail,
        progress: progress.map(|value| value.clamp(0, 100) as u8),
        // A state the table cannot hold does not exist: the `CHECK` refuses it on the way in.
        state: TaskState::parse(&state).unwrap_or(TaskState::Running),
        pausable: pausable != 0,
        cancellable: cancellable != 0,
        created_at,
        updated_at,
    }
}

fn validate(id: &str, source: &str, name: &str, detail: &str) -> Result<(), ApiFailure> {
    let identifier = |value: &str, max: usize| {
        !value.is_empty()
            && value.len() <= max
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    if !identifier(id, 64) || !identifier(source, 64) || name.trim().is_empty() || name.len() > 160
    {
        return Err(ApiFailure::bad("invalid task"));
    }
    if detail.len() > 500 {
        return Err(ApiFailure::bad("the detail of this task is too long"));
    }
    Ok(())
}

/// The register, running ones first and then the most recently finished.
pub async fn list(db: &SqlitePool) -> Result<Vec<Task>, ApiFailure> {
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, source, name, detail, progress, state, pausable, cancellable,
                created_at, updated_at
         FROM tasks
         ORDER BY state IN ('running', 'paused') DESC, updated_at DESC, id",
    )
    .fetch_all(db)
    .await
    .map_err(|_| unavailable())?;
    Ok(rows.into_iter().map(row).collect())
}

pub async fn get(db: &SqlitePool, id: &str) -> Result<Task, ApiFailure> {
    let found: Option<Row> = sqlx::query_as(
        "SELECT id, source, name, detail, progress, state, pausable, cancellable,
                created_at, updated_at
         FROM tasks WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .map_err(|_| unavailable())?;
    found.map(row).ok_or_else(|| ApiFailure::missing("task"))
}

/// Registers or replaces an operation with the same ID, preserving its creation time.
pub async fn register(db: &SqlitePool, input: NewTask) -> Result<Task, ApiFailure> {
    validate(&input.id, &input.source, &input.name, &input.detail)?;
    let at = now_ms();
    sqlx::query(
        "INSERT INTO tasks
            (id, source, name, detail, progress, state, pausable, cancellable, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, 'running', ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
            source = excluded.source,
            name = excluded.name,
            detail = excluded.detail,
            progress = excluded.progress,
            state = 'running',
            pausable = excluded.pausable,
            cancellable = excluded.cancellable,
            updated_at = excluded.updated_at",
    )
    .bind(&input.id)
    .bind(&input.source)
    .bind(input.name.trim())
    .bind(&input.detail)
    .bind(input.progress.map(|value| i64::from(value.min(100))))
    .bind(i64::from(input.pausable))
    .bind(i64::from(input.cancellable))
    .bind(at)
    .bind(at)
    .execute(db)
    .await
    .map_err(|_| unavailable())?;

    prune(db).await;
    get(db, &input.id).await
}

/// What the owner reports while it works. A finished operation cannot be moved again: the owner
/// registers a new one instead of reopening a row the user has already read.
pub async fn update(db: &SqlitePool, id: &str, input: TaskUpdate) -> Result<Task, ApiFailure> {
    update_with_notification(db, id, input, true).await
}

/// Initial provisioning reports failures on the preparation page and in its installation log.
pub(crate) async fn update_without_notification(
    db: &SqlitePool,
    id: &str,
    input: TaskUpdate,
) -> Result<Task, ApiFailure> {
    update_with_notification(db, id, input, false).await
}

async fn update_with_notification(
    db: &SqlitePool,
    id: &str,
    input: TaskUpdate,
    notify: bool,
) -> Result<Task, ApiFailure> {
    let current = get(db, id).await?;
    if !current.state.active() {
        return Err(ApiFailure::bad("this task has already finished"));
    }
    if input
        .detail
        .as_ref()
        .is_some_and(|detail| detail.len() > 500)
    {
        return Err(ApiFailure::bad("the detail of this task is too long"));
    }
    if input
        .name
        .as_ref()
        .is_some_and(|name| name.trim().is_empty() || name.len() > 160)
    {
        return Err(ApiFailure::bad("invalid task"));
    }

    let state = input.state.unwrap_or(current.state);
    let saved = write_state(
        db,
        id,
        state,
        input
            .name
            .as_deref()
            .map(str::trim)
            .unwrap_or(&current.name),
        input.detail.as_deref().unwrap_or(&current.detail),
        input.progress.unwrap_or(current.progress),
    )
    .await?;

    // A failure is the one outcome the user must not miss, so it also goes to the log.
    if state == TaskState::Failed && notify {
        let message = if saved.detail.is_empty() {
            format!("Operation “{}” not completed.", saved.name)
        } else {
            format!("Operation “{}” not completed. {}", saved.name, saved.detail)
        };
        let mut notice = Notice::new("taskFailed", "Operation failed", message)
            .with("name", saved.name.clone())
            .with("reason", saved.detail.clone());
        if saved.source == "nas-app-manager" && saved.id.starts_with("install-") {
            notice = notice.log(operation_log(&format!("{}.log", saved.id)));
        } else if saved.source == "nas-disk-manager" && saved.id == "initialize-storage" {
            notice = notice.log(operation_log("initialize-storage.log"));
        } else {
            notice = notice
                .detail(
                    &format!("task-{}-{}", saved.source, saved.id),
                    &saved.detail,
                    None,
                )
                .await;
        }
        notifications::record(db, &saved.source, Tone::Error, notice).await;
    }
    Ok(saved)
}

/// Stores pause/resume/cancel requests only when the owner supports them.
/// The owner is responsible for acting on the stored state.
pub async fn request(db: &SqlitePool, id: &str, action: &str) -> Result<Task, ApiFailure> {
    let current = get(db, id).await?;
    if !current.state.active() {
        return Err(ApiFailure::bad("this task has already finished"));
    }

    let state = match action {
        "pause" if current.pausable => TaskState::Paused,
        "resume" if current.pausable => TaskState::Running,
        "stop" if current.cancellable => TaskState::Cancelled,
        "pause" | "resume" | "stop" => {
            return Err(ApiFailure::bad("this task cannot be asked for that"));
        }
        _ => return Err(ApiFailure::bad("invalid task action")),
    };

    write_state(
        db,
        id,
        state,
        &current.name,
        &current.detail,
        current.progress,
    )
    .await
}

async fn write_state(
    db: &SqlitePool,
    id: &str,
    state: TaskState,
    name: &str,
    detail: &str,
    progress: Option<u8>,
) -> Result<Task, ApiFailure> {
    sqlx::query(
        "UPDATE tasks SET state = ?, name = ?, detail = ?, progress = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(state.as_str())
    .bind(name)
    .bind(detail)
    .bind(progress.map(i64::from))
    .bind(now_ms())
    .bind(id)
    .execute(db)
    .await
    .map_err(|_| unavailable())?;
    let saved = get(db, id).await?;
    if !saved.state.active() {
        prune(db).await;
    }
    Ok(saved)
}

/// Removes one operation, whatever state it is in.
pub async fn remove(db: &SqlitePool, id: &str) -> Result<(), ApiFailure> {
    let removed = sqlx::query("DELETE FROM tasks WHERE id = ?")
        .bind(id)
        .execute(db)
        .await
        .map_err(|_| unavailable())?;
    if removed.rows_affected() == 0 {
        return Err(ApiFailure::missing("task"));
    }
    Ok(())
}

/// Clears everything that has finished, which is the button at the head of the widget.
pub async fn clear_finished(db: &SqlitePool) -> Result<u64, ApiFailure> {
    Ok(
        sqlx::query("DELETE FROM tasks WHERE state NOT IN ('running', 'paused')")
            .execute(db)
            .await
            .map_err(|_| unavailable())?
            .rows_affected(),
    )
}

/// Keeps the newest finished operation for each source, name and detail, then at most `LIMIT`.
/// Running and paused rows are never removed by retention.
pub async fn prune(db: &SqlitePool) {
    let swept = sqlx::query(
        "WITH finished AS (
             SELECT rowid, updated_at,
                    ROW_NUMBER() OVER (
                        PARTITION BY source, name, detail
                        ORDER BY updated_at DESC, rowid DESC
                    ) AS duplicate
             FROM tasks WHERE state NOT IN ('running', 'paused')
         ), kept AS (
             SELECT rowid FROM finished WHERE duplicate = 1
             ORDER BY updated_at DESC, rowid DESC LIMIT ?
         )
         DELETE FROM tasks
         WHERE state NOT IN ('running', 'paused')
           AND (updated_at < ? OR rowid NOT IN (SELECT rowid FROM kept))",
    )
    .bind(LIMIT)
    .bind(now_ms() - KEEP_FINISHED_MS)
    .execute(db)
    .await;
    if let Err(error) = swept {
        eprintln!("task register not pruned: {error}");
    }
}

fn unavailable() -> ApiFailure {
    ApiFailure::unavailable("the task register is unavailable")
}

#[cfg(test)]
#[path = "../../../../tests/widgets/services/tasks.rs"]
mod tests;
