//! Validated application permissions beyond the default systemd sandbox.
//!
//! Manifests request paths and network capabilities from [`GRANTABLE`]. Requests
//! outside this list are rejected. Validated grants are stored in `app-grants/<id>`.
//! The privileged `nas-os-app-grants@<id>.service` helper maps these entries to a
//! fixed systemd drop-in through `apply-app-grants.sh`.

use crate::{
    services::{database, lifecycle, manifest::Manifest},
    utils::{self, Mode},
};
use std::{path::PathBuf, time::Duration};

/// Allowed data-disk, hardware, SMTP, and local-sync permissions.
/// Includes reduced runtime priority for applications with background work.
pub const GRANTABLE: [&str; 7] = [
    "/srv/storage",
    "/proc",
    "/sys",
    "/run/udev",
    "network:smtp",
    "network:sync",
    "run:background",
];
const COMMAND_TIMEOUT: Duration = Duration::from_secs(20);

/// The paths of a manifest the launcher grants, once each and in a fixed order, and those it does
/// not.
pub fn split(asked: &[String]) -> (Vec<&'static str>, Vec<String>) {
    let granted = GRANTABLE
        .iter()
        .copied()
        .filter(|path| asked.iter().any(|wanted| wanted == path))
        .collect();
    let refused = asked
        .iter()
        .filter(|wanted| !GRANTABLE.contains(&wanted.as_str()))
        .cloned()
        .collect();
    (granted, refused)
}

/// Validates permissions requested by a package before installation.
pub fn checked(asked: &[String]) -> Result<Vec<&'static str>, String> {
    let (granted, refused) = split(asked);
    if refused.is_empty() {
        Ok(granted)
    } else {
        Err(format!(
            "the package asks for {}, which no application is given",
            refused.join(", ")
        ))
    }
}

/// The request file `apply-app-grants.sh` reads.
pub fn request_text(granted: &[&str]) -> String {
    granted.iter().map(|path| format!("{path}\n")).collect()
}

fn requests_directory() -> Option<PathBuf> {
    database::beside("app-grants")
}

fn unit_of(id: &str) -> String {
    format!("nas-os-app-grants@{id}.service")
}

/// The drop-in the helper writes for an application.
fn dropin_of(id: &str) -> PathBuf {
    std::env::var_os("NAS_APP_DROPINS")
        .map_or_else(|| PathBuf::from("/etc/systemd/system"), PathBuf::from)
        .join(format!("nas-app@{id}.service.d"))
        .join("grants.conf")
}

/// Checks whether the installed drop-in matches the requested grants.
/// Unapplied requests remain eligible for retry.
pub fn in_place(nothing_granted: bool, dropin_exists: bool) -> bool {
    nothing_granted != dropin_exists
}

async fn run(id: &str) -> Result<(), String> {
    utils::systemctl(&["start", &unit_of(id)], COMMAND_TIMEOUT)
        .await
        .map_err(|reason| format!("the grants of {id} were not applied: {reason}"))
}

/// Persists and applies changed application grants.
/// Returns whether the drop-in changed. Running applications require a restart.
pub async fn apply(id: &str, granted: &[&str]) -> Result<bool, String> {
    if !utils::is_real(Mode::Apps) || !crate::services::manifest::is_identifier(id) {
        return Ok(false);
    }
    let directory = requests_directory().ok_or("no state directory for the grants")?;
    let file = directory.join(id);
    let text = request_text(granted);
    let requested = tokio::fs::read_to_string(&file)
        .await
        .is_ok_and(|current| current == text);
    let written = tokio::fs::try_exists(dropin_of(id)).await.unwrap_or(false);
    if requested && in_place(granted.is_empty(), written) {
        return Ok(false);
    }
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    tokio::fs::write(&file, text)
        .await
        .map_err(|error| format!("cannot write {}: {error}", file.display()))?;
    if let Err(reason) = run(id).await {
        // Asked again at the next start, instead of looking applied.
        let _ = tokio::fs::remove_file(&file).await;
        return Err(reason);
    }
    Ok(true)
}

/// Takes back what a removed application was granted.
pub async fn revoke(id: &str) -> Result<(), String> {
    if !utils::is_real(Mode::Apps) || !crate::services::manifest::is_identifier(id) {
        return Ok(());
    }
    if let Some(directory) = requests_directory() {
        let _ = tokio::fs::remove_file(directory.join(id)).await;
    }
    run(id).await
}

/// Reconciles installed application grants at startup.
/// Restarts running applications after changes and logs entries no longer allowed.
pub async fn sync(manifests: &[Manifest]) {
    for manifest in manifests {
        let (granted, refused) = split(&manifest.grants_asked());
        if !refused.is_empty() {
            eprintln!(
                "{} asks for {}, which no application is given: not granted",
                manifest.id,
                refused.join(", ")
            );
        }
        match apply(&manifest.id, &granted).await {
            Ok(true) if lifecycle::is_running(&manifest.id).await => {
                let unit = format!("nas-app@{}.service", manifest.id);
                if let Err(reason) =
                    utils::systemctl(&["try-restart", &unit], COMMAND_TIMEOUT).await
                {
                    eprintln!("{unit} keeps its old grants until restarted: {reason}");
                }
            }
            Ok(_) => {}
            Err(reason) => eprintln!("{reason}"),
        }
    }
}

#[cfg(test)]
#[path = "../../tests/services/grants.rs"]
mod tests;
