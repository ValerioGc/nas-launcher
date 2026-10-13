//! The folders an application shares on the local network, with Samba (SMB, which Windows and
//! Linux open as `\\nas\name` and `smb://nas/name`).
//!
//! An application that declares `permissions.shares` in its manifest, and is granted the data
//! disks, sends the whole list of its shares on its socket ([`crate::controllers::app_channel`]).
//! The launcher checks every line and writes them in `smb-shares/<id>` beside the database; then
//! `nas-os-smb-shares.service` runs `apply-smb-shares.sh` as root, which checks them again, writes
//! the configuration of Samba, keeps `smbd` running only while a folder is shared, and opens TCP
//! 445 to the local network only then (the set `nas_shares` of the base firewall). The password of
//! a protected share is the one of the account: the gateway hands it to Samba, never the launcher.

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

/// The data disks as the applications see them: a share is a folder below.
const STORAGE_ROOT: &str = "/srv/storage";
const MAX_SHARES: usize = 64;
const UNIT: &str = "nas-os-smb-shares.service";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
/// What Samba itself answers for, which no share may be called.
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
    /// Where a file deleted from the share goes: the trash of its disk at the path of the share
    /// (`<disk>/.nas-trash/<folder>`), for the `recycle` module of Samba. None deletes for good.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trash: Option<String>,
}

/// The shares of one application, all of them at once.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shares {
    pub shares: Vec<Share>,
}

/// What an application needs to know to offer its shares.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sharing {
    /// Samba is installed on the NAS.
    pub available: bool,
    /// The user a computer signs in with to a protected share.
    pub user: Option<String>,
    /// A share may carry its `trash`: an older launcher refuses the field.
    pub recycle: bool,
}

/// A name of a share as Windows and Samba both take it: letters, digits, spaces and `_ - .`, at
/// most 64, never one of Samba's own sections.
pub fn is_name(name: &str) -> bool {
    (1..=64).contains(&name.chars().count())
        && name.trim() == name
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || " _-.".contains(c))
        && !RESERVED.contains(&name.to_lowercase().as_str())
}

/// A folder of the data disks, written so that the configuration of Samba reads it as it is: no
/// `..`, no empty segment nor one with a space at either end, which Samba would trim, no control
/// character and no `%`, which Samba would expand.
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

/// Whether `trash` is the place of a shared folder in the trash of its disk: the folder is
/// `<disk>/<folder>` and the trash `<disk>/.nas-trash/<folder>` (or `<disk>/.nas-trash` for a
/// whole disk).
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

/// The request file `apply-smb-shares.sh` reads: one share per line, its fields apart by tabs.
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

/// Whether the manifest may share folders: it asks for it, and for the data disks.
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

/// What an application says it shares, all of it: written and applied when it changed.
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

/// At start, and after an application that shares was installed: Samba and the firewall as the
/// requests say, since the set of the firewall is empty after a boot and a new Samba starts with
/// the configuration of Debian. Only on a NAS where an application may share.
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
#[path = "../../../tests/services/shares.rs"]
mod tests;
