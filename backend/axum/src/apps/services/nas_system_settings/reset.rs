//! Factory reset preserving data disks and restarting the setup wizard.
//!
//! The launcher requires gateway confirmation of the account password before
//! starting `nas-os-reset.service`. Its privileged `factory-reset.sh` helper
//! removes state and stops the requesting launcher during execution.

use crate::{
    http::error::ApiFailure,
    services::{
        database,
        packages::{credentials, forge},
    },
    utils::{self, Mode},
};

use super::updates::{self, Component};
use serde::Serialize;
use std::{os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};
use tokio::{sync::Mutex, task::JoinHandle};

pub const UNIT: &str = "nas-os-reset.service";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a confirmed reset waits for the download of a newer wizard before it goes without.
const DOWNLOAD_WAIT: Duration = Duration::from_secs(5);
/// Operations a reset must not interrupt. Keep in step with factory-reset.sh and uninstall.sh of
/// the wizard, which also waits for this unit.
const BUSY_UNITS: [&str; 8] = [
    "nas-os-update.service",
    "nas-os-update-verify.service",
    "nas-os-apt@*.service",
    "nas-os-app-packages@*.service",
    "nas-os-storage-initialize.service",
    "nas-os-storage-rename.service",
    "nas-os-storage-eject.service",
    "nas-os-storage-crypt@*.service",
];

/// The download of a newer wizard, started once when the reset is first shown.
static PREPARING: Mutex<Option<JoinHandle<()>>> = Mutex::const_new(None);
/// The run of the unit before this launcher started it, so an older failure is never taken for
/// the answer to this request.
static REQUESTED: Mutex<Option<String>> = Mutex::const_new(None);

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
                "The wizard is not installed on this machine. Run install.sh from its package once."
            }
            Self::NoOriginalState => {
                "The original system configuration was not saved; an exact reset is unavailable."
            }
        }
    }
}

/// Whether the NAS can be reset here, why not when it cannot, and how a requested reset goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResetState {
    pub available: bool,
    pub reason: Option<ResetBlock>,
    /// The reset unit is running.
    pub running: bool,
    /// The reset this launcher started stopped before it removed anything, the journal of the
    /// unit says why.
    pub failed: bool,
}

/// What systemd says of the last run of the reset unit.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct UnitRun {
    pub active: String,
    pub result: String,
    pub invocation: String,
}

impl UnitRun {
    /// Reads `systemctl show` in its `key=value` form, which lists properties in its own order.
    pub fn parse(listing: &str) -> Self {
        let mut run = Self::default();
        for (key, value) in listing.lines().filter_map(|line| line.split_once('=')) {
            let value = value.trim().to_owned();
            match key {
                "ActiveState" => run.active = value,
                "Result" => run.result = value,
                "InvocationID" => run.invocation = value,
                _ => {}
            }
        }
        run
    }

    pub fn running(&self) -> bool {
        matches!(
            self.active.as_str(),
            "active" | "activating" | "deactivating" | "reloading"
        )
    }

    /// Failed after the run seen as `requested`, the one before this launcher started the unit.
    pub fn failed_since(&self, requested: Option<&str>) -> bool {
        requested.is_some_and(|before| self.invocation != before)
            && !self.running()
            && (self.active == "failed" || !matches!(self.result.as_str(), "" | "success"))
    }
}

/// Installed setup-wizard unit started after reset.
fn wizard_unit() -> PathBuf {
    utils::env_path("NAS_WIZARD_UNIT", "/etc/systemd/system/nas-wizard.service")
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
        running: false,
        failed: false,
    }
}

pub async fn state() -> ResetState {
    let unit_installed = tokio::fs::metadata(wizard_unit())
        .await
        .is_ok_and(|unit| unit.is_file());
    let home = utils::env_path("NAS_WIZARD_HOME", "/usr/local/lib/nas-wizard");
    let binary_installed = tokio::fs::metadata(home.join("wizard/bin/nas-wizard"))
        .await
        .is_ok_and(|binary| binary.is_file() && binary.permissions().mode() & 0o111 != 0);
    let helper_installed =
        tokio::fs::metadata(home.join("packages/os_configuration/scripts/uninstall.sh"))
            .await
            .is_ok_and(|helper| helper.is_file() && helper.permissions().mode() & 0o111 != 0);
    let configuration = utils::env_path("NAS_CONFIGURATION_STATE", "/var/lib/nas-os-configuration");
    let original_saved = tokio::fs::metadata(configuration.join("original-ready"))
        .await
        .is_ok_and(|marker| marker.is_file());
    let mut state = availability(
        utils::is_real(Mode::Reset),
        unit_installed && binary_installed && helper_installed,
        original_saved,
    );
    if state.available {
        prepare_in_background().await;
        if let Some(run) = unit_run().await {
            state.running = run.running();
            state.failed = run.failed_since(REQUESTED.lock().await.as_deref());
        }
    }
    state
}

async fn unit_run() -> Option<UnitRun> {
    let listing = utils::systemctl_output(
        &["show", "--property=ActiveState,Result,InvocationID", UNIT],
        COMMAND_TIMEOUT,
    )
    .await
    .ok()?;
    Some(UnitRun::parse(&listing))
}

/// The marker an update leaves until its check after the restart, which would put the previous
/// version back in the middle of a reset.
async fn update_pending() -> bool {
    match database::beside("update-verify/pending") {
        Some(marker) => tokio::fs::try_exists(marker).await.unwrap_or(false),
        None => false,
    }
}

/// Starts the reset, once the gateway has confirmed the password of the account.
pub async fn start() -> Result<ResetState, ApiFailure> {
    let current = state().await;
    if let Some(reason) = current.reason {
        return Err(ApiFailure::unavailable(reason.explain()));
    }
    let mut listing = vec![
        "list-units",
        "--plain",
        "--no-legend",
        "--state=active,activating,deactivating,reloading",
    ];
    listing.extend(BUSY_UNITS);
    let busy = utils::systemctl_output(&listing, COMMAND_TIMEOUT)
        .await
        .map_err(|reason| {
            ApiFailure::unavailable(format!("cannot check active NAS operations ({reason})"))
        })?;
    if !busy.trim().is_empty() {
        return Err(ApiFailure::unavailable(
            "Wait for NAS updates, package installation or disk operations before resetting.",
        ));
    }
    if update_pending().await {
        return Err(ApiFailure::unavailable(
            "An update waits for its check after the restart. Restart the NAS first.",
        ));
    }
    finish_download().await;
    *REQUESTED.lock().await = Some(unit_run().await.unwrap_or_default().invocation);
    // Queues the helper without waiting because it stops the requesting launcher.
    utils::systemctl(&["start", "--no-block", UNIT], COMMAND_TIMEOUT)
        .await
        .map_err(|reason| ApiFailure::unavailable(format!("the reset did not start ({reason})")))?;
    Ok(current)
}

/// Starts the download of a newer wizard once, so a confirmed reset rarely has to wait for it.
async fn prepare_in_background() {
    let mut preparing = PREPARING.lock().await;
    if preparing.is_none() {
        *preparing = Some(tokio::spawn(prepare_wizard()));
    }
}

/// Gives a running download a few seconds, then drops it, so the reset never finds half an
/// archive and goes on with the installed wizard.
async fn finish_download() {
    let mut preparing = PREPARING.lock().await;
    let Some(download) = preparing.as_mut() else {
        return;
    };
    if download.is_finished()
        || tokio::time::timeout(DOWNLOAD_WAIT, &mut *download)
            .await
            .is_ok()
    {
        return;
    }
    download.abort();
    let _ = (&mut *download).await;
    if let Some(archive) = database::beside(&Component::Wizard.archive()) {
        let _ = tokio::fs::remove_file(archive).await;
    }
    eprintln!("the newer wizard was still downloading, the reset uses the one installed");
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
