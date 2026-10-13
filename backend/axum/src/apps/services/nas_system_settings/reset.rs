//! The reset of the NAS: launcher, gateway and applications back to where the wizard found them,
//! the data disks untouched, and the wizard started again.
//!
//! The launcher only asks: `factory-reset.sh`, as root in `nas-os-reset.service`, does the work,
//! and stops the launcher along the way. The password of the account is the gateway's to check:
//! the controller takes the request only with its confirmation (`http::extract::Reauthenticated`).

use crate::{
    http::error::ApiFailure,
    services::packages::{credentials, forge},
    utils::{self, Mode},
};

use super::updates::{self, Component};
use serde::Serialize;
use std::{path::PathBuf, time::Duration};

pub const UNIT: &str = "nas-os-reset.service";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// Why the NAS cannot be reset here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ResetBlock {
    /// `NAS_RESET_MODE` is not `real`: development, or an installation that turned it off
    Disabled,
    /// The unit of the wizard is not on the machine, so nothing would configure the NAS again
    NoWizard,
}

impl ResetBlock {
    fn explain(self) -> &'static str {
        match self {
            Self::Disabled => "The reset is not available on this installation.",
            Self::NoWizard => {
                "the wizard is not installed on this machine: run install.sh from its package once"
            }
        }
    }
}

/// Whether the NAS can be reset here, and why not when it cannot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResetState {
    pub available: bool,
    pub reason: Option<ResetBlock>,
}

/// The unit the reset starts at the end: the wizard, installed on the machine with it.
fn wizard_unit() -> PathBuf {
    std::env::var_os("NAS_WIZARD_UNIT").map_or_else(
        || PathBuf::from("/etc/systemd/system/nas-wizard.service"),
        PathBuf::from,
    )
}

pub fn availability(real: bool, wizard_installed: bool) -> ResetState {
    let reason = if !real {
        Some(ResetBlock::Disabled)
    } else if !wizard_installed {
        Some(ResetBlock::NoWizard)
    } else {
        None
    };
    ResetState {
        available: reason.is_none(),
        reason,
    }
}

pub async fn state() -> ResetState {
    let installed = tokio::fs::metadata(wizard_unit())
        .await
        .is_ok_and(|unit| unit.is_file());
    availability(utils::is_real(Mode::Reset), installed)
}

/// Starts the reset, once the gateway has confirmed the password of the account.
pub async fn start() -> Result<ResetState, ApiFailure> {
    let current = state().await;
    if let Some(reason) = current.reason {
        return Err(ApiFailure::unavailable(reason.explain()));
    }
    prepare_wizard().await;
    // Not waited for: the unit stops this very launcher.
    utils::systemctl(&["start", "--no-block", UNIT], COMMAND_TIMEOUT)
        .await
        .map_err(|reason| ApiFailure::unavailable(format!("the reset did not start: {reason}")))?;
    Ok(current)
}

/// Downloads a newer wizard, when there is one, for `factory-reset.sh` to install before it starts
/// the wizard: the NAS is configured again with the latest. Anything that goes wrong leaves the
/// wizard installed as it is, and the reset goes on.
async fn prepare_wizard() {
    if !utils::is_real(Mode::Packages) {
        return;
    }
    let Some(current) = Component::Wizard.installed().await else {
        return;
    };
    let repository = Component::Wizard.repository();
    if repository.is_empty() {
        return;
    }
    let token = credentials::launcher().await;
    let provider = updates::provider();
    let newer = match forge::latest(provider, &repository, token.as_deref()).await {
        Ok(release) if updates::is_update(&release.version, current.as_deref()) => release.version,
        Ok(_) => return,
        Err(reason) => {
            eprintln!("the wizard is reset as it is: {reason}");
            return;
        }
    };
    if let Err(reason) = updates::download(Component::Wizard, &newer).await {
        eprintln!(
            "the wizard {newer} was not downloaded, the reset uses the one installed: {reason}"
        );
    }
}

#[cfg(test)]
#[path = "../../../../../tests/apps/services/nas_system_settings/reset.rs"]
mod tests;
