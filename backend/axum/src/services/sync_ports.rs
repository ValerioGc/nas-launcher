//! The ports of the sync with the computers of the local network: an application whose manifest
//! asks for `network:sync` (nas-sync-manager) says on its socket whether it has a sync on
//! (`PUT /api/apps/self/sync`), and the launcher has the base firewall open the ports of Syncthing to
//! the local network meanwhile. The launcher writes `on` in `sync-ports/<id>` beside the database,
//! or removes the file, and starts `nas-os-sync-ports.service`, which runs `apply-sync-ports.sh` as
//! root: it puts the fixed ports in the set `nas_sync`, or empties it. Polkit lets it start nothing
//! else (62-nas-os-sync-ports.rules).

use crate::{
    http::error::ApiFailure,
    services::{
        database,
        manifest::{self, Manifest},
    },
    utils::{self, Mode},
};
use std::{
    future::Future,
    path::{Path, PathBuf},
    time::Duration,
};

static APPLY: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const UNIT: &str = "nas-os-sync-ports.service";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Whether the manifest may open the ports of the sync.
pub fn may_sync(manifest: &Manifest) -> bool {
    manifest.network.iter().any(|kind| kind == "sync")
}

fn requests_directory() -> Option<PathBuf> {
    database::beside("sync-ports")
}

async fn run() -> Result<(), ApiFailure> {
    utils::systemctl(&["start", UNIT], COMMAND_TIMEOUT)
        .await
        .map_err(|reason| {
            ApiFailure::unavailable(format!("the sync ports were not applied: {reason}"))
        })
}

/// What an application says of its syncs: persisted and reapplied, including failed attempts.
pub async fn declare(id: &str, active: bool) -> Result<(), ApiFailure> {
    if !manifest::read(id).await.is_some_and(|app| may_sync(&app)) {
        return Err(ApiFailure::forbidden(format!(
            "{id} does not declare network:sync in its manifest"
        )));
    }
    if !utils::is_real(Mode::Apps) {
        return Ok(());
    }
    let directory = requests_directory()
        .ok_or_else(|| ApiFailure::unavailable("no state directory for the sync ports"))?;
    apply_request(&directory, id, active, run()).await
}

async fn apply_request(
    directory: &Path,
    id: &str,
    active: bool,
    apply: impl Future<Output = Result<(), ApiFailure>>,
) -> Result<(), ApiFailure> {
    let _guard = APPLY.lock().await;
    let file = directory.join(id);
    // Reapply even if the request is unchanged: an earlier helper failure must be retried.
    if active {
        tokio::fs::create_dir_all(&directory)
            .await
            .map_err(|error| ApiFailure::unavailable(format!("cannot keep the sync: {error}")))?;
        tokio::fs::write(&file, "on")
            .await
            .map_err(|error| ApiFailure::unavailable(format!("cannot keep the sync: {error}")))?;
    } else {
        match tokio::fs::remove_file(&file).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ApiFailure::unavailable(format!(
                    "cannot close the sync: {error}"
                )))
            }
        }
    }
    apply.await
}

/// At start, and after an application that syncs was installed: the set of the firewall is empty
/// after a boot. Only on a NAS where an application may sync.
pub async fn sync(manifests: &[Manifest]) {
    if manifests.iter().any(may_sync) {
        refresh().await;
    }
}

/// The ports as the requests say, with a failure only in the journal.
pub async fn refresh() {
    if !utils::is_real(Mode::Apps) {
        return;
    }
    if let Err(failure) = run().await {
        eprintln!("<4>{}", failure.1);
    }
}

/// Closes what a removed application had open.
pub async fn revoke(id: &str) {
    if !utils::is_real(Mode::Apps) || !manifest::is_identifier(id) {
        return;
    }
    let Some(file) = requests_directory().map(|directory| directory.join(id)) else {
        return;
    };
    if tokio::fs::remove_file(&file).await.is_ok() {
        refresh().await;
    }
}

#[cfg(test)]
#[path = "../../../tests/services/sync_ports.rs"]
mod tests;
