//! Persists one JSON document per settings section without interpreting its fields.

use crate::{
    http::error::ApiFailure,
    services::notifications::{self, Notice, Tone},
    utils::now_ms,
};
use serde::{de::DeserializeOwned, Serialize};
use sqlx::SqlitePool;

const LOCAL_USER: &str = "local";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    General,
    Notifications,
    Access,
    Power,
    Widgets,
    Terminal,
    AppManager,
    Catalogue,
    Antivirus,
    Discovery,
    Network,
}
impl Section {
    pub const ALL: [Section; 11] = [
        Section::General,
        Section::Notifications,
        Section::Access,
        Section::Power,
        Section::Widgets,
        Section::Terminal,
        Section::AppManager,
        Section::Catalogue,
        Section::Antivirus,
        Section::Discovery,
        Section::Network,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Section::General => "general",
            Section::Notifications => "notifications",
            Section::Access => "access",
            Section::Power => "power",
            Section::Widgets => "widgets",
            Section::Terminal => "terminal",
            Section::AppManager => "apps",
            Section::Catalogue => "catalogue",
            Section::Antivirus => "antivirus",
            Section::Discovery => "discovery",
            Section::Network => "network",
        }
    }

    fn position(self) -> i64 {
        Self::ALL
            .iter()
            .position(|section| *section == self)
            .unwrap_or_default() as i64
    }
}

pub async fn read<T>(db: &SqlitePool, section: Section) -> Result<T, ApiFailure>
where
    T: DeserializeOwned + Default,
{
    let stored: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE user_name = ? AND section = ?")
            .bind(LOCAL_USER)
            .bind(section.id())
            .fetch_optional(db)
            .await
            .map_err(|_| unavailable(section))?;
    let Some((document,)) = stored else {
        return Ok(T::default());
    };

    Ok(serde_json::from_str(&document).unwrap_or_else(|error| {
        eprintln!("unreadable settings section {}: {error}", section.id());
        T::default()
    }))
}

/// Writes one section, on the pool or inside a transaction that writes others with it.
pub async fn write<'e, T, E>(db: E, section: Section, value: &T) -> Result<(), ApiFailure>
where
    T: Serialize,
    E: sqlx::SqliteExecutor<'e>,
{
    let document = serde_json::to_string(value).map_err(|_| unavailable(section))?;
    sqlx::query(
        "INSERT INTO settings (user_name, section, position, value, updated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT (user_name, section) DO UPDATE
         SET value = excluded.value, position = excluded.position, updated_at = excluded.updated_at",
    )
    .bind(LOCAL_USER)
    .bind(section.id())
    .bind(section.position())
    .bind(document)
    .bind(now_ms())
    .execute(db)
    .await
    .map_err(|_| unavailable(section))?;
    Ok(())
}

/// How long the notice of a saved section stands for the saves that follow it: a value changed
/// step by step, or saved as it is typed, leaves one notice and not one for each step.
const SAVED_NOTICE_MS: i64 = 30_000;

pub async fn record_saved(db: &SqlitePool, section: Section) {
    let params = serde_json::json!({ "section": section.id() }).to_string();
    let refreshed = sqlx::query(
        "UPDATE notifications SET created_at = ?1 WHERE id = (
             SELECT id FROM notifications
             WHERE key = 'settingsSaved' AND params = ?2 AND read = 0 AND created_at >= ?3
             ORDER BY id DESC LIMIT 1)",
    )
    .bind(now_ms())
    .bind(&params)
    .bind(now_ms() - SAVED_NOTICE_MS)
    .execute(db)
    .await;
    if refreshed.is_ok_and(|done| done.rows_affected() > 0) {
        return;
    }
    notifications::record(
        db,
        "nas-system-settings",
        Tone::Success,
        Notice::new(
            "settingsSaved",
            "Settings saved",
            format!("The {} settings were updated.", section.id()),
        )
        .with("section", section.id()),
    )
    .await;
}

fn unavailable(section: Section) -> ApiFailure {
    ApiFailure::unavailable(format!("settings section unavailable: {}", section.id()))
}

#[cfg(test)]
#[path = "../../../tests/services/settings.rs"]
mod tests;
