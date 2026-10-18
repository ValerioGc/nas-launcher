//! Application folder shares served through Samba.
//!
//! Applications request `permissions.shares` and data-disk access, then submit their
//! complete share list through the application socket. The launcher validates and
//! persists requests in `smb-shares/<id>`. A privileged helper revalidates them,
//! configures Samba, and opens TCP 445 locally while shares are active.
//! The gateway supplies account credentials directly to Samba for protected shares.

use crate::{
    http::error::ApiFailure,
    services::{
        database,
        manifest::{self, Manifest},
        storage,
    },
    utils::{self, Mode},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf, time::Duration};

/// Root of application-visible data disks containing shared folders.
const STORAGE_ROOT: &str = "/srv/storage";
const MAX_SHARES: usize = 64;
const UNIT: &str = "nas-os-smb-shares.service";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
/// Reserved Samba names excluded from application shares.
const RESERVED: [&str; 5] = ["global", "homes", "printers", "print$", "ipc$"];

/// One folder shared on the local network.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Share {
    /// The name the computers of the network see.
    pub name: String,
    /// The folder, under `/srv/storage`.
    pub path: String,
    /// Written as well as read.
    pub writable: bool,
    /// Opened only with the user and the password of the account; otherwise by anyone.
    pub protected: bool,
    /// Samba recycle destination within the data disk's trash.
    /// Deleted files retain the shared folder path under `<disk>/.nas-trash`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trash: Option<String>,
}

/// The shares of one application, all of them at once.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shares {
    pub shares: Vec<Share>,
}

/// Sharing capability and account metadata exposed to applications.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sharing {
    /// Samba is installed on the NAS.
    pub available: bool,
    /// The user a computer signs in with to a protected share.
    pub user: Option<String>,
    /// Includes optional trash metadata rejected by older launcher versions.
    pub recycle: bool,
}

/// Validates share names accepted by Windows and Samba.
/// Allows up to 64 letters, digits, spaces, underscores, hyphens, or periods.
/// Reserved Samba section names are excluded.
pub fn is_name(name: &str) -> bool {
    (1..=64).contains(&name.chars().count())
        && name.trim() == name
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || " _-.".contains(c))
        && !RESERVED.contains(&name.to_lowercase().as_str())
}

/// Validates a data-disk path for literal use in Samba configuration.
/// Rejects traversal, empty segments, edge spaces, control characters, and `%` expansion.
pub fn is_path(path: &str) -> bool {
    path.len() <= 4096
        && path
            .strip_prefix(STORAGE_ROOT)
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some_and(|rest| {
                !rest.is_empty()
                    && rest.split('/').all(|segment| {
                        !segment.is_empty()
                            && segment != "."
                            && segment != ".."
                            && segment.trim() == segment
                    })
            })
        && !path.chars().any(|c| c.is_control() || c == '%')
}

/// Checks that a recycle destination matches the shared folder within its disk trash.
pub fn is_trash_of(trash: &str, path: &str) -> bool {
    const MARK: &str = "/.nas-trash";
    let Some(at) = trash.find(MARK) else {
        return false;
    };
    let (disk, rest) = trash.split_at(at);
    let rest = &rest[MARK.len()..];
    let expected = match rest.strip_prefix('/') {
        _ if rest.is_empty() => disk.to_owned(),
        Some(folder) => format!("{disk}/{folder}"),
        None => return false,
    };
    is_path(trash) && !rest.contains(".nas-trash") && expected == path
}

/// Every share of the list, checked, and the names and the folders each used once.
pub fn checked(shares: &[Share]) -> Result<(), ApiFailure> {
    if shares.len() > MAX_SHARES {
        return Err(ApiFailure::bad(format!(
            "at most {MAX_SHARES} folders can be shared"
        )));
    }
    let mut names = HashSet::new();
    let mut paths = HashSet::new();
    for share in shares {
        if !is_name(&share.name) {
            return Err(ApiFailure::bad(format!(
                "{} is not a name a share can have",
                share.name
            )));
        }
        if !is_path(&share.path) {
            return Err(ApiFailure::bad(format!(
                "{} is not a folder of the data disks",
                share.path
            )));
        }
        if share
            .trash
            .as_deref()
            .is_some_and(|trash| !is_trash_of(trash, &share.path))
        {
            return Err(ApiFailure::bad(format!(
                "{} is not the trash of the disk of {}",
                share.trash.as_deref().unwrap_or_default(),
                share.name
            )));
        }
        if !names.insert(share.name.to_lowercase()) || !paths.insert(share.path.as_str()) {
            return Err(ApiFailure::bad(format!("{} is shared twice", share.name)));
        }
    }
    Ok(())
}

/// Writes the tab-separated share records consumed by `apply-smb-shares.sh`.
pub fn request_text(shares: &[Share]) -> String {
    shares
        .iter()
        .map(|share| {
            format!(
                "{}\t{}\t{}\t{}\t{}\n",
                share.name,
                share.path,
                if share.writable { "rw" } else { "ro" },
                if share.protected { "protected" } else { "open" },
                share.trash.as_deref().unwrap_or("-")
            )
        })
        .collect()
}

/// Checks for both sharing permission and data-disk access in the manifest.
pub fn may_share(manifest: &Manifest) -> bool {
    manifest.shares && manifest.paths.iter().any(|path| path == STORAGE_ROOT)
}

fn requests_directory() -> Option<PathBuf> {
    database::beside("smb-shares")
}

async fn run() -> Result<(), ApiFailure> {
    utils::systemctl(&["start", UNIT], COMMAND_TIMEOUT)
        .await
        .map_err(|reason| {
            ApiFailure::unavailable(format!("the shared folders were not applied: {reason}"))
        })
}

/// Persists and applies changed application share requests.
pub async fn declare(id: &str, shares: &[Share]) -> Result<(), ApiFailure> {
    if !manifest::read(id).await.is_some_and(|app| may_share(&app)) {
        return Err(ApiFailure::forbidden(format!(
            "{id} does not declare permissions.shares with the data disks in its manifest"
        )));
    }
    checked(shares)?;
    if !utils::is_real(Mode::Apps) {
        return Ok(());
    }
    let directory = requests_directory()
        .ok_or_else(|| ApiFailure::unavailable("no state directory for the shared folders"))?;
    let file = directory.join(id);
    let text = request_text(shares);
    if tokio::fs::read_to_string(&file)
        .await
        .is_ok_and(|current| current == text)
    {
        return Ok(());
    }
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| ApiFailure::unavailable(format!("cannot create the shares: {error}")))?;
    tokio::fs::write(&file, text)
        .await
        .map_err(|error| ApiFailure::unavailable(format!("cannot write the shares: {error}")))?;
    if let Err(failure) = run().await {
        // Asked again at the next declaration, instead of looking applied.
        let _ = tokio::fs::remove_file(&file).await;
        return Err(failure);
    }
    Ok(())
}

/// Whether Samba is on the NAS, and who signs in to a protected share.
pub async fn sharing() -> Sharing {
    let samba = std::env::var_os("NAS_SAMBA_SMBD")
        .map_or_else(|| PathBuf::from("/usr/sbin/smbd"), PathBuf::from);
    Sharing {
        available: tokio::fs::try_exists(&samba).await.unwrap_or(false),
        user: storage::account_user().await.ok().flatten(),
        recycle: true,
    }
}

/// Reconciles requested shares after boot and application installation.
pub async fn sync(manifests: &[Manifest]) {
    if manifests.iter().any(may_share) {
        refresh().await;
    }
}

/// Samba and the firewall as the requests say, with a failure only in the journal.
pub async fn refresh() {
    if !utils::is_real(Mode::Apps) {
        return;
    }
    if let Err(failure) = run().await {
        eprintln!("<4>{}", failure.1);
    }
}

/// Takes back what a removed application shared.
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
#[path = "../../tests/services/shares.rs"]
mod tests;
