//! The paths and the network an application may reach beyond the sandbox of `nas-app@.service`.
//!
//! The template hides from every application the data disks, the state of the launcher, the logs,
//! `/sys`, `/run/udev` and the processes of others, and lets it out only on HTTP and HTTPS. A
//! manifest asks for more in `permissions.paths` and `permissions.network` (here `network:<kind>`);
//! the launcher grants only what [`GRANTABLE`] lists, and a package asking for anything else is not
//! installed. What is granted is written, one path per line, in
//! `app-grants/<id>` beside the database, and `nas-os-app-grants@<id>.service` turns it into the
//! drop-in `nas-app@<id>.service.d/grants.conf`, as root, through `apply-app-grants.sh`. That script
//! knows the same list and decides what each path becomes: the launcher only says which.

use crate::{
    services::{database, lifecycle, manifest::Manifest},
    utils::{self, Mode},
};
use std::{path::PathBuf, time::Duration};

/// What an application may be given: the data disks, read and written, the kernel's views of the
/// hardware, read only, outgoing mail on the SMTP ports, the sync with the local network on the
/// ports of Syncthing, and the lower priority of an application that works without its window.
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

/// What a package about to be installed is granted, or why it is not installed.
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

/// Whether the helper already did what the request says: a drop-in when something is granted,
/// none otherwise. A request written while the helper could not apply it is applied again.
pub fn in_place(nothing_granted: bool, dropin_exists: bool) -> bool {
    nothing_granted != dropin_exists
}

async fn run(id: &str) -> Result<(), String> {
    utils::systemctl(&["start", &unit_of(id)], COMMAND_TIMEOUT)
        .await
        .map_err(|reason| format!("the grants of {id} were not applied: {reason}"))
}

/// Writes what an application is granted and has it applied, when it changed. True when the
/// drop-in changed: a running application sees it only once restarted.
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

/// At start, the grants of every installed application, for the packages installed before the
/// grants existed and after a change of this list. An application whose grants changed is
/// restarted when it is running. A path not in the list is left out and said in the journal: the
/// package was installed by an older launcher.
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
#[path = "../../../tests/services/grants.rs"]
mod tests;
