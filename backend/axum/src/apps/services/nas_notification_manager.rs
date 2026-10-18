use crate::{
    http::error::ApiFailure,
    services::{
        notifications::{self, Notification, Row, Tone},
        settings::{self, Section},
    },
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

pub mod log;

#[derive(Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NotificationSettings {
    pub email_enabled: bool,
    pub email: String,
    pub webhook_enabled: bool,
    pub webhook: String,
}
impl NotificationSettings {
    pub fn validate(&self) -> Result<(), ApiFailure> {
        if self.email.len() > 254
            || self.webhook.len() > 500
            || self.email.contains(['\r', '\n'])
            || (self.email_enabled && (!self.email.contains('@') || self.email.contains(' ')))
            || (self.webhook_enabled
                && (!self.webhook.starts_with("https://")
                    || self.webhook.contains([' ', '\r', '\n'])))
        {
            return Err(ApiFailure::bad("invalid notification settings"));
        }
        Ok(())
    }
}

pub async fn load_settings(db: &SqlitePool) -> Result<NotificationSettings, ApiFailure> {
    settings::read(db, Section::Notifications).await
}

pub async fn save_settings(
    db: &SqlitePool,
    input: NotificationSettings,
) -> Result<NotificationSettings, ApiFailure> {
    input.validate()?;
    settings::write(db, Section::Notifications, &input).await?;
    settings::record_saved(db, Section::Notifications).await;
    Ok(input)
}

#[derive(Debug, Default, Deserialize, utoipa::IntoParams)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct NotificationFilter {
    pub tone: Option<String>,
    pub source: Option<String>,
    pub read: Option<bool>,
    pub q: Option<String>,
    pub limit: Option<usize>,
}
impl NotificationFilter {
    pub fn validate(&self) -> Result<(), ApiFailure> {
        if self.limit.is_some_and(|n| n == 0 || n > 500)
            || self.q.as_ref().is_some_and(|s| s.len() > 200)
            || self.source.as_ref().is_some_and(|s| s.len() > 64)
            || self.tone.as_ref().is_some_and(|s| Tone::parse(s).is_none())
        {
            return Err(ApiFailure::bad("invalid notification filter"));
        }
        Ok(())
    }
}

pub async fn list(
    db: &SqlitePool,
    filter: &NotificationFilter,
) -> Result<Vec<Notification>, ApiFailure> {
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, source, title, message, tone, read, created_at, key, params, log
         FROM notifications
         WHERE (?1 IS NULL OR tone = ?1)
           AND (?2 IS NULL OR source = ?2)
           AND (?3 IS NULL OR read = ?3)
         ORDER BY id DESC",
    )
    .bind(filter.tone.as_deref())
    .bind(filter.source.as_deref())
    .bind(filter.read.map(i64::from))
    .fetch_all(db)
    .await
    .map_err(|_| notifications::unavailable())?;

    let needle = filter
        .q
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_lowercase);

    Ok(rows
        .into_iter()
        .map(notifications::row)
        .filter(|item| {
            needle.as_ref().is_none_or(|needle| {
                format!("{} {}", item.content.title, item.content.message)
                    .to_lowercase()
                    .contains(needle)
            })
        })
        .take(filter.limit.unwrap_or(100))
        .collect())
}

pub async fn sources(db: &SqlitePool) -> Result<Vec<String>, ApiFailure> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT DISTINCT source FROM notifications ORDER BY source")
            .fetch_all(db)
            .await
            .map_err(|_| notifications::unavailable())?;
    Ok(rows.into_iter().map(|(source,)| source).collect())
}

pub async fn mark_read(db: &SqlitePool, id: &str, read: bool) -> Result<Notification, ApiFailure> {
    let key = key(id)?;
    let row: Option<Row> = sqlx::query_as(
        "UPDATE notifications SET read = ? WHERE id = ?
         RETURNING id, source, title, message, tone, read, created_at, key, params, log",
    )
    .bind(i64::from(read))
    .bind(key)
    .fetch_optional(db)
    .await
    .map_err(|_| notifications::unavailable())?;

    row.map(notifications::row)
        .ok_or_else(|| ApiFailure::missing("notification"))
}

/// Marks all unread notifications as read and returns how many it changed.
pub async fn mark_all_read(db: &SqlitePool) -> Result<u64, ApiFailure> {
    Ok(
        sqlx::query("UPDATE notifications SET read = 1 WHERE read = 0")
            .execute(db)
            .await
            .map_err(|_| notifications::unavailable())?
            .rows_affected(),
    )
}

/// Removes every notification that has already been read.
pub async fn remove_read(db: &SqlitePool) -> Result<u64, ApiFailure> {
    Ok(sqlx::query("DELETE FROM notifications WHERE read = 1")
        .execute(db)
        .await
        .map_err(|_| notifications::unavailable())?
        .rows_affected())
}

pub async fn get(db: &SqlitePool, id: &str) -> Result<Notification, ApiFailure> {
    let row: Option<Row> = sqlx::query_as(
        "SELECT id, source, title, message, tone, read, created_at, key, params, log
         FROM notifications WHERE id = ?",
    )
    .bind(key(id)?)
    .fetch_optional(db)
    .await
    .map_err(|_| notifications::unavailable())?;
    row.map(notifications::row)
        .ok_or_else(|| ApiFailure::missing("notification"))
}

pub async fn remove(db: &SqlitePool, id: &str) -> Result<(), ApiFailure> {
    let removed = sqlx::query("DELETE FROM notifications WHERE id = ?")
        .bind(key(id)?)
        .execute(db)
        .await
        .map_err(|_| notifications::unavailable())?;
    if removed.rows_affected() == 0 {
        return Err(ApiFailure::missing("notification"));
    }
    Ok(())
}

fn key(id: &str) -> Result<i64, ApiFailure> {
    id.parse().map_err(|_| ApiFailure::missing("notification"))
}
