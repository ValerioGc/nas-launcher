//! The shares of other computers and NAS of the local network, mounted as network disks (SMB).
//!
//! What the person sets (server, share, user, domain, whether it connects by itself) root keeps in
//! `/etc/nas-os/network-disks.json`, the password as the encrypted credential `nas-os.smb-<id>` of
//! systemd. The launcher hands every change to the root helper of the data disks
//! ([`super::crypt`]) on its socket, and never mounts anything itself. The reconciliation mounts
//! the shares that connect by themselves, unless the person disconnected them, on
//! `/srv/storage/<id>` with the user of the wizard as owner, and publishes them to Explorer, the
//! backups and the sync as `network` volumes. The kernel reaches the server on TCP 445, which the
//! base firewall lets out only towards the addresses of the local network the root helper put in
//! its sets `nas_smb_servers` and `nas_smb_servers6`. Needs `NAS_NETWORK_DISKS_MODE=real`, the
//! default of `production.env`.

pub mod root;

use super::{crypt, inventory};
use crate::{
    http::error::ApiFailure,
    utils::{self, Mode},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What the launcher asks of the root helper about the network disks.
#[derive(Debug, Clone, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum NetworkRequest {
    /// Adds a share, or changes one; without a password a change keeps the one it had.
    #[serde(rename_all = "camelCase")]
    NetworkSave {
        #[serde(default)]
        id: Option<String>,
        name: String,
        server: String,
        share: String,
        username: String,
        #[serde(default)]
        domain: Option<String>,
        #[serde(default)]
        password: Option<String>,
        automatic: bool,
    },
    /// Tries a share without keeping it.
    #[serde(rename_all = "camelCase")]
    NetworkTest {
        server: String,
        share: String,
        username: String,
        #[serde(default)]
        domain: Option<String>,
        password: String,
    },
    /// Mounts a share now; it stays connected until the person disconnects it.
    NetworkConnect { id: String },
    /// Unmounts a share, which stays disconnected until the person connects it again.
    NetworkDisconnect { id: String },
    /// Unmounts a share and forgets it, password included.
    NetworkForget { id: String },
}

/// A share as root keeps it: no password.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Definition {
    pub id: String,
    pub name: String,
    pub server: String,
    pub share: String,
    pub username: String,
    #[serde(default)]
    pub domain: Option<String>,
    pub automatic: bool,
}

/// A share as the disk manager shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkDisk {
    #[serde(flatten)]
    pub definition: Definition,
    /// Mounted now.
    pub connected: bool,
    /// Disconnected by the person: nothing connects it but the person.
    pub held: bool,
}

/// The shares, and whether this installation mounts them at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct NetworkDisks {
    pub available: bool,
    pub disks: Vec<NetworkDisk>,
}

const MAX_DISKS: usize = 32;

pub(super) fn definitions_path() -> PathBuf {
    std::env::var_os("NAS_NETWORK_DISKS").map_or_else(
        || PathBuf::from("/etc/nas-os/network-disks.json"),
        PathBuf::from,
    )
}

pub(super) fn definitions() -> Vec<Definition> {
    std::fs::read(definitions_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Where a share is mounted: a folder of Explorer of its own.
pub(super) fn target(id: &str) -> PathBuf {
    std::path::Path::new(super::EXPLORER_ROOT).join(id)
}

pub(super) fn valid_id(id: &str) -> bool {
    id.strip_prefix("n-").is_some_and(|hex| {
        (8..=32).contains(&hex.len()) && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn plain(value: &str, longest: usize) -> bool {
    (1..=longest).contains(&value.chars().count())
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

/// A server as the kernel and the firewall take it: a name or an address, nothing else.
pub(super) fn valid_server(server: &str) -> bool {
    (1..=253).contains(&server.len())
        && !server.starts_with(['-', '.'])
        && server
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b':'))
}

fn valid_share(share: &str) -> bool {
    plain(share, 80) && !share.contains(['/', '\\'])
}

fn valid_domain(domain: &str) -> bool {
    (1..=64).contains(&domain.len())
        && domain
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

/// A name of Explorer: one line of the registry, so no tab and no line break.
fn valid_name(name: &str) -> bool {
    plain(name, 40)
}

fn valid_password(password: &str) -> bool {
    (1..=256).contains(&password.chars().count()) && !password.chars().any(char::is_control)
}

fn account(
    server: &str,
    share: &str,
    username: &str,
    domain: Option<&String>,
) -> Result<(), String> {
    if !valid_server(server) {
        return Err("invalid server".to_owned());
    }
    if !valid_share(share) {
        return Err("invalid share name".to_owned());
    }
    if !plain(username, 64) {
        return Err("invalid user name".to_owned());
    }
    if domain.is_some_and(|domain| !valid_domain(domain)) {
        return Err("invalid domain".to_owned());
    }
    Ok(())
}

/// The request checked on its own, before anything looks at the machine.
pub fn validate(request: &NetworkRequest) -> Result<(), String> {
    match request {
        NetworkRequest::NetworkSave {
            id,
            name,
            server,
            share,
            username,
            domain,
            password,
            ..
        } => {
            if id.as_deref().is_some_and(|id| !valid_id(id)) {
                return Err("invalid network disk".to_owned());
            }
            if !valid_name(name) {
                return Err("invalid name".to_owned());
            }
            if id.is_none() && password.is_none() {
                return Err("a new network disk needs its password".to_owned());
            }
            if password
                .as_deref()
                .is_some_and(|password| !valid_password(password))
            {
                return Err("invalid password".to_owned());
            }
            account(server, share, username, domain.as_ref())
        }
        NetworkRequest::NetworkTest {
            server,
            share,
            username,
            domain,
            password,
        } => {
            if !valid_password(password) {
                return Err("invalid password".to_owned());
            }
            account(server, share, username, domain.as_ref())
        }
        NetworkRequest::NetworkConnect { id }
        | NetworkRequest::NetworkDisconnect { id }
        | NetworkRequest::NetworkForget { id } => {
            if valid_id(id) {
                Ok(())
            } else {
                Err("invalid network disk".to_owned())
            }
        }
    }
}

/// Whether this installation mounts network disks.
pub fn available() -> bool {
    utils::is_real(Mode::NetworkDisks)
}

/// The shares, connected or not, as root last wrote them and the host mounts show them.
pub fn list() -> Result<NetworkDisks, String> {
    let mountinfo = std::fs::read_to_string("/proc/1/mountinfo")
        .or_else(|_| std::fs::read_to_string("/proc/self/mountinfo"))
        .map_err(|error| format!("cannot read the mounts of the system: {error}"))?;
    let mounted = inventory::mountpoints(&mountinfo);
    let held = crypt::root::held();
    Ok(NetworkDisks {
        available: available(),
        disks: definitions()
            .into_iter()
            .map(|definition| NetworkDisk {
                connected: mounted.contains(&target(&definition.id)),
                held: held.contains(&definition.id),
                definition,
            })
            .collect(),
    })
}

/// Checks the request and has the root helper run it.
pub async fn request(request: NetworkRequest) -> Result<crypt::Answer, ApiFailure> {
    validate(&request).map_err(ApiFailure::bad)?;
    if !available() {
        return Err(ApiFailure::unavailable(
            "Network disks are not available on this installation.",
        ));
    }
    if matches!(request, NetworkRequest::NetworkSave { id: None, .. })
        && definitions().len() >= MAX_DISKS
    {
        return Err(ApiFailure::bad(format!(
            "at most {MAX_DISKS} network disks"
        )));
    }
    crypt::send(&request, std::time::Duration::from_secs(120))
        .await
        .map_err(ApiFailure::unavailable)
}

#[cfg(test)]
#[path = "../../../../tests/services/storage/network.rs"]
mod tests;
