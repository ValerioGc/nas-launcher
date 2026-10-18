//! Factory reset preserving data disks and restarting the setup wizard.
//!
//! The launcher requires gateway confirmation of the account password before
//! starting `nas-os-reset.service`. Its privileged `factory-reset.sh` helper
//! removes state and stops the requesting launcher during execution.

use crate::{
    http::error::ApiFailure,
    services::packages::{credentials, forge},
    utils::{self, Mode},
};

use super::updates::{self, Component};
use serde::Serialize;
use std::{os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};
use tokio::process::Command;

pub const UNIT: &str = "nas-os-reset.service";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// Why the NAS cannot be reset here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ResetBlock {
    /// Reset operations are disabled because `NAS_RESET_MODE` is not `real`.
    Disabled,
    /// The unit of the wizard is not on the machine, so nothing would configure the NAS again
    NoWizard,
    /// The state from before the installation was never saved
    NoOriginalState,
}

impl ResetBlock {
    fn explain(self) -> &'static str {
        match self {
            Self::Disabled => "The reset is not available on this installation.",
            Self::NoWizard => {
                "the wizard is not installed on this machine: run install.sh from its package once"
            }
            Self::NoOriginalState => {
                "The original system configuration was not saved; an exact reset is unavailable."
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

/// Installed setup-wizard unit started after reset.
fn wizard_unit() -> PathBuf {
    std::env::var_os("NAS_WIZARD_UNIT").map_or_else(
        || PathBuf::from("/etc/systemd/system/nas-wizard.service"),
        PathBuf::from,
    )
}

pub fn availability(real: bool, wizard_installed: bool, original_saved: bool) -> ResetState {
    let reason = if !real {
        Some(ResetBlock::Disabled)
    } else if !wizard_installed {
        Some(ResetBlock::NoWizard)
    } else if !original_saved {
        Some(ResetBlock::NoOriginalState)
    } else {
        None
    };
    ResetState {
        available: reason.is_none(),
        reason,
    }
}

pub async fn state() -> ResetState {
    let unit_installed = tokio::fs::metadata(wizard_unit())
        .await
        .is_ok_and(|unit| unit.is_file());
    let home = std::env::var_os("NAS_WIZARD_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/local/lib/nas-wizard"));
    let binary_installed = tokio::fs::metadata(home.join("wizard/bin/nas-wizard"))
        .await
        .is_ok_and(|binary| binary.is_file() && binary.permissions().mode() & 0o111 != 0);
    let helper_installed =
        tokio::fs::metadata(home.join("packages/os_configuration/scripts/uninstall.sh"))
            .await
            .is_ok_and(|helper| helper.is_file() && helper.permissions().mode() & 0o111 != 0);
    let configuration = std::env::var_os("NAS_CONFIGURATION_STATE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/lib/nas-os-configuration"));
    let original_saved = tokio::fs::metadata(configuration.join("original-ready"))
        .await
        .is_ok_and(|marker| marker.is_file());
    availability(
        utils::is_real(Mode::Reset),
        unit_installed && binary_installed && helper_installed,
        original_saved,
    )
}

/// Starts the reset, once the gateway has confirmed the password of the account.
pub async fn start() -> Result<ResetState, ApiFailure> {
    let current = state().await;
    if let Some(reason) = current.reason {
        return Err(ApiFailure::unavailable(reason.explain()));
    }
    let listing = Command::new("systemctl")
        .args([
            "list-units",
            "--plain",
            "--no-legend",
            "--state=active,activating,deactivating,reloading",
            "nas-os-update.service",
            "nas-os-apt@*.service",
            "nas-os-app-packages@*.service",
            "nas-os-storage-initialize.service",
            "nas-os-storage-rename.service",
            "nas-os-storage-eject.service",
        ])
        .output();
    let busy = tokio::time::timeout(COMMAND_TIMEOUT, listing)
        .await
        .map_err(|_| ApiFailure::unavailable("Checking active NAS operations timed out."))?
        .map_err(|reason| {
            ApiFailure::unavailable(format!("cannot check active NAS operations: {reason}"))
        })?;
    if !busy.status.success() {
        return Err(ApiFailure::unavailable(format!(
            "cannot check active NAS operations: {}",
            String::from_utf8_lossy(&busy.stderr).trim()
        )));
    }
    if !String::from_utf8_lossy(&busy.stdout).trim().is_empty() {
        return Err(ApiFailure::unavailable(
            "Wait for NAS updates, package installation or disk operations before resetting.",
        ));
    }
    prepare_wizard().await;
    // Queues the helper without waiting because it stops the requesting launcher.
    utils::systemctl(&["start", "--no-block", UNIT], COMMAND_TIMEOUT)
        .await
        .map_err(|reason| ApiFailure::unavailable(format!("the reset did not start: {reason}")))?;
    Ok(current)
}

/// Prepares an updated wizard for reset, falling back to the installed wizard if preparation
/// fails.
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
#[path = "../../../../tests/apps/services/nas_system_settings/reset.rs"]
mod tests;
