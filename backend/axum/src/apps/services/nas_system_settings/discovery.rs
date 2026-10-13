//! The discovery of the NAS by NAS Finder on the local network (UDP 39877), an option of the
//! security center. The wizard answers while it runs and closes the port when it hands over to the
//! gateway: from then on the option is off, and the end of the first configuration records a
//! notification saying so. The gateway always answers; the base firewall decides whether a query
//! reaches it. While the option is on, the launcher writes `on` in `discovery` beside the database
//! (and removes the file when it is off) and starts `nas-os-discovery.service`, which runs
//! `apply-discovery.sh` as root: it puts the fixed port in the set `nas_discovery`, or empties it.
//! Polkit lets it start nothing else (63-nas-os-discovery.rules).

use crate::{
    http::error::ApiFailure,
    services::{
        database,
        notifications::{self, Notice, Tone},
        settings::{self, Section},
    },
    utils::{self, Mode},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{future::Future, path::PathBuf, time::Duration};

static APPLY: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const UNIT: &str = "nas-os-discovery.service";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
/// The application the option lives in, which the notification names.
const APP: &str = "nas-security-center";

#[derive(Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Discovery {
    pub enabled: bool,
}

pub async fn load(db: &SqlitePool) -> Result<Discovery, ApiFailure> {
    settings::read(db, Section::Discovery).await
}

/// The option as asked: the port opened or closed first, then kept.
pub async fn save(db: &SqlitePool, input: Discovery) -> Result<Discovery, ApiFailure> {
    apply(input.enabled).await?;
    settings::write(db, Section::Discovery, &input).await?;
    settings::record_saved(db, Section::Discovery).await;
    Ok(input)
}

async fn run() -> Result<(), ApiFailure> {
    utils::systemctl(&["start", UNIT], COMMAND_TIMEOUT)
        .await
        .map_err(|reason| {
            ApiFailure::unavailable(format!("the discovery was not applied: {reason}"))
        })
}

async fn apply(enabled: bool) -> Result<(), ApiFailure> {
    if !utils::is_real(Mode::Firewall) {
        return Ok(());
    }
    apply_request(database::beside("discovery"), enabled, run()).await
}

async fn apply_request(
    file: Option<PathBuf>,
    enabled: bool,
    apply: impl Future<Output = Result<(), ApiFailure>>,
) -> Result<(), ApiFailure> {
    let file =
        file.ok_or_else(|| ApiFailure::unavailable("no state directory for the discovery"))?;
    let _guard = APPLY.lock().await;
    if enabled {
        tokio::fs::write(&file, "on").await.map_err(|error| {
            ApiFailure::unavailable(format!("cannot keep the discovery: {error}"))
        })?;
    } else {
        match tokio::fs::remove_file(&file).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ApiFailure::unavailable(format!(
                    "cannot close the discovery: {error}"
                )))
            }
        }
    }
    apply.await
}

/// At start: the set of the firewall is empty after a boot, so an option left on opens the port
/// again. One that is off changes nothing, so the launcher never closes the port while the wizard
/// still answers there.
pub async fn apply_saved(db: &SqlitePool) {
    if !load(db).await.is_ok_and(|discovery| discovery.enabled) {
        return;
    }
    if let Err(failure) = apply(true).await {
        eprintln!("<4>{}", failure.1);
    }
}

/// At the end of the first configuration: the wizard closed the port, and the user is told where
/// to open it again.
pub async fn announce_off(db: &SqlitePool) {
    let notice = Notice::new(
        "discoveryOff",
        "Network discovery off",
        "For security the NAS can no longer be found on the local network by NAS Finder, now that \
         its first configuration is done. It stays so until the system is reset; Security Center \
         can turn it on again in its settings.",
    );
    notifications::record(db, APP, Tone::Info, notice).await;
}

#[cfg(test)]
#[path = "../../../../../tests/apps/services/nas_system_settings/discovery.rs"]
mod tests;
