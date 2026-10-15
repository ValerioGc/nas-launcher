//! The Debian packages an application needs on the machine, installed before it and removed after.
//!
//! A manifest asks for them in `permissions.packages`; the launcher accepts only the packages of
//! [`INSTALLABLE`], and a package asking for anything else is not installed. What an application
//! asked for is written, one package per line, in `app-packages/<id>` beside the database, and
//! `nas-os-app-packages@<id>.service` runs `app-packages.sh` as root: it installs what that
//! application asks for and is missing, and purges what it installed earlier and no application
//! asks for any more. A package that was on the machine before is never removed.

use crate::{
    services::{database, manifest},
    utils::{self, Mode},
};
use std::{path::PathBuf, time::Duration};

/// Free software an application may have installed: SMTP delivery, ClamAV, 7-Zip, Samba, the
/// server of the folders shared on the local network, Syncthing, the engine of the sync, and
/// cifs-utils, which mounts the network disks.
pub const INSTALLABLE: [&str; 7] = [
    "msmtp",
    "clamav",
    "clamav-freshclam",
    "7zip",
    "samba",
    "syncthing",
    "cifs-utils",
];
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// The packages a manifest may have installed, once each and in a fixed order, or why it may not.
pub fn checked(asked: &[String]) -> Result<Vec<&'static str>, String> {
    let refused: Vec<_> = asked
        .iter()
        .filter(|wanted| !INSTALLABLE.contains(&wanted.as_str()))
        .cloned()
        .collect();
    if !refused.is_empty() {
        return Err(format!(
            "the package asks to install {}, which no application is given",
            refused.join(", ")
        ));
    }
    Ok(INSTALLABLE
        .iter()
        .copied()
        .filter(|name| asked.iter().any(|wanted| wanted == name))
        .collect())
}

fn requests_directory() -> Option<PathBuf> {
    database::beside("app-packages")
}

async fn run(id: &str) -> Result<(), String> {
    let unit = format!("nas-os-app-packages@{id}.service");
    utils::systemctl(&["start", &unit], COMMAND_TIMEOUT)
        .await
        .map_err(|reason| format!("the system packages of {id} were not set up: {reason}"))
}

/// Has what an application asks for installed, before the application takes its place. An
/// application asking for nothing leaves the machine as it is.
pub async fn install(id: &str, packages: &[&str]) -> Result<(), String> {
    if packages.is_empty() || !utils::is_real(Mode::Apps) || !manifest::is_identifier(id) {
        return Ok(());
    }
    let directory = requests_directory().ok_or("no state directory for the system packages")?;
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    let text: String = packages.iter().map(|name| format!("{name}\n")).collect();
    let file = directory.join(id);
    tokio::fs::write(&file, text)
        .await
        .map_err(|error| format!("cannot write {}: {error}", file.display()))?;
    run(id).await
}

/// Takes back what a removed application had installed, unless another one still asks for it.
pub async fn remove(id: &str) -> Result<(), String> {
    if !utils::is_real(Mode::Apps) || !manifest::is_identifier(id) {
        return Ok(());
    }
    let Some(file) = requests_directory().map(|directory| directory.join(id)) else {
        return Ok(());
    };
    if tokio::fs::remove_file(&file).await.is_err() {
        return Ok(());
    }
    run(id).await
}

#[cfg(test)]
#[path = "../../../tests/services/system_packages.rs"]
mod tests;
