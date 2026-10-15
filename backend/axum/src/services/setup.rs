//! The end of the first configuration of the NAS.
//!
//! The wizard is a project of its own, which runs as root before the launcher serves anybody: it
//! stores the release token, creates the user of the terminal and the account of the gateway, then
//! hands the launcher what is the launcher's, the name of the NAS, language and theme, on the
//! loopback. From there the launcher installs the applications the catalogue wants from the start.

use crate::{
    apps::services::nas_system_settings::{apply_hostname, discovery, General, Language, Theme},
    http::error::ApiFailure,
    services::{
        notifications::{self, Notice, Tone},
        provisioning::{self, Step},
        settings::{self, Section},
    },
    utils::now_ms,
};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::Path;

/// Where the entry script of the wizard notes that it installed the NAS in development mode,
/// without an encrypted system or a TPM 2.0.
const DEVELOPMENT_MARK: &str = "/var/lib/nas-wizard/security-development";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct SetupState {
    pub completed: bool,
    pub ready: bool,
    pub steps: Vec<Step>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct FirstConfiguration {
    pub language: Language,
    pub theme: Theme,
    /// The name chosen in the wizard, which becomes the hostname; without it the default stays
    #[serde(default)]
    pub nas_name: Option<String>,
}

pub async fn state(db: &SqlitePool) -> Result<SetupState, ApiFailure> {
    let completed: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM setup)")
        .fetch_one(db)
        .await
        .map_err(|_| unavailable())?;
    let steps = provisioning::steps(db).await?;
    Ok(SetupState {
        completed,
        ready: completed && provisioning::finished(&steps),
        steps,
    })
}

pub async fn still_open(db: &SqlitePool) -> Result<(), ApiFailure> {
    if state(db).await?.completed {
        return Err(ApiFailure(
            StatusCode::CONFLICT,
            "the NAS is already configured".into(),
        ));
    }
    Ok(())
}

pub async fn complete(
    db: &SqlitePool,
    input: FirstConfiguration,
) -> Result<SetupState, ApiFailure> {
    still_open(db).await?;

    let mut general: General = settings::read(db, Section::General).await?;
    general.language = input.language;
    general.theme = input.theme;
    if let Some(name) = input.nas_name {
        general.nas_name = name.trim().to_owned();
        general.validate()?;
        apply_hostname(&general.nas_name).await?;
    }
    settings::write(db, Section::General, &general).await?;
    provisioning::plan(db).await?;
    sqlx::query("INSERT OR IGNORE INTO setup (id, completed_at) VALUES (1, ?)")
        .bind(now_ms())
        .execute(db)
        .await
        .map_err(|_| unavailable())?;
    discovery::announce_off(db).await;
    // The wizard may have encrypted disks with their automatic key alone.
    crate::services::storage::crypt::report_spare_keys(db).await;

    tokio::spawn(provisioning::resume(db.clone()));
    state(db).await
}

fn unavailable() -> ApiFailure {
    ApiFailure::unavailable("the setup state cannot be read")
}

/// Warns at every start, while the warning before is unread, that the NAS was installed without
/// the security it needs.
pub async fn report_development(db: &SqlitePool) {
    report_development_mark(db, Path::new(DEVELOPMENT_MARK)).await;
}

async fn report_development_mark(db: &SqlitePool, mark: &Path) {
    if tokio::fs::metadata(mark).await.is_err() {
        return;
    }
    let unread: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM notifications WHERE key = 'developmentMode' AND read = 0)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(true);
    if !unread {
        notifications::record(
            db,
            "nas-security-center",
            Tone::Warning,
            Notice::new(
                "developmentMode",
                "NAS in development mode",
                "This NAS was installed without an encrypted system or a TPM 2.0: whoever takes one of its disks can read the data on it.",
            ),
        )
        .await;
    }
}

#[cfg(test)]
#[path = "../../../tests/services/setup.rs"]
mod tests;
