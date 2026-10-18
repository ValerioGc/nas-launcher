//! Launcher provisioning after the setup wizard completes.
//!
//! The wizard configures credentials, the terminal account, and gateway access.
//! It transfers the NAS name, language, and appearance over loopback. The launcher
//! then installs applications marked for initial installation in the catalogue.

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

/// System-encryption and TPM status detected by the wizard during installation or updates.
const SECURITY_STATE: &str = "/var/lib/nas-wizard/security";

/// Whether the system is on LUKS and there is a TPM 2.0, as the wizard found them; `None` when it
/// did not say (a development copy, or a NAS installed before it wrote the state).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SystemSecurity {
    pub system_encrypted: Option<bool>,
    pub tpm: Option<bool>,
}

impl SystemSecurity {
    /// Something is known to be missing.
    pub fn unprotected(self) -> bool {
        self.system_encrypted == Some(false) || self.tpm == Some(false)
    }
}

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

fn parse_security(text: &str) -> SystemSecurity {
    let mut state = SystemSecurity::default();
    for line in text.lines() {
        match line.trim().split_once('=') {
            Some(("system", value)) => {
                state.system_encrypted = match value {
                    "encrypted" => Some(true),
                    "plain" => Some(false),
                    _ => None,
                }
            }
            Some(("tpm", value)) => {
                state.tpm = match value {
                    "present" => Some(true),
                    "missing" => Some(false),
                    _ => None,
                }
            }
            _ => {}
        }
    }
    state
}

async fn read_security(path: &Path) -> SystemSecurity {
    tokio::fs::read_to_string(path)
        .await
        .map(|text| parse_security(&text))
        .unwrap_or_default()
}

/// The security of the machine as the wizard found it.
pub async fn system_security() -> SystemSecurity {
    read_security(Path::new(SECURITY_STATE)).await
}

/// Records the initial system-encryption or TPM status warning once.
pub async fn report_security(db: &SqlitePool) {
    report_security_state(db, Path::new(SECURITY_STATE)).await;
}

async fn report_security_state(db: &SqlitePool, path: &Path) {
    if !read_security(path).await.unprotected() {
        return;
    }
    let told: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM notifications WHERE key = 'systemUnprotected')",
    )
    .fetch_one(db)
    .await
    .unwrap_or(true);
    if !told {
        notifications::record(
            db,
            "nas-security-center",
            Tone::Warning,
            Notice::new(
                "systemUnprotected",
                "System disk not protected",
                "The system of this NAS is not on an encrypted disk, or there is no TPM 2.0: whoever takes its disks away can read the NAS on them. The installation manual explains what this means.",
            ),
        )
        .await;
    }
}

#[cfg(test)]
#[path = "../../tests/services/setup.rs"]
mod tests;
