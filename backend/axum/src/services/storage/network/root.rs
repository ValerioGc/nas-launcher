//! The root half of the network disks: the requests of the socket, and the shares the
//! reconciliation mounts. A password goes to `mount.cifs` in a credentials file of the runtime
//! directory of the unit, removed once mounted, never in an argument.

use super::{definitions, definitions_path, target, validate, Definition, NetworkRequest};
use crate::services::storage::{
    crypt::{
        root::{credstore, held, random, raw_output, set_held, users_of, Secret},
        Answer,
    },
    output, output_logged, owner, reconcile, Grant,
};
use std::{
    fs,
    io::Write,
    net::IpAddr,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

/// How long one mount may take, in seconds, for `timeout`.
const MOUNT_WITHIN: &str = "20";

fn log_path() -> PathBuf {
    std::env::var_os("NAS_LOG_DIR")
        .map_or_else(|| PathBuf::from("/var/log/nas-os"), PathBuf::from)
        .join("network-disks.log")
}

fn credential_name(id: &str) -> String {
    format!("nas-os.smb-{id}")
}

fn credential(id: &str) -> PathBuf {
    credstore().join(credential_name(id))
}

/// An address the base firewall lets the kernel reach: the private and link-local ranges.
pub(super) fn is_local(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
    }
}

/// The addresses of `getent ahosts`, once each.
pub(super) fn addresses_of(listing: &str) -> Vec<IpAddr> {
    let mut found = Vec::new();
    for address in listing
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter_map(|field| field.parse::<IpAddr>().ok())
    {
        if !found.contains(&address) {
            found.push(address);
        }
    }
    found
}

/// The addresses of the server on the local network, and the base firewall told to let the
/// kernel reach them on 445.
fn open_to(server: &str) -> Result<Vec<IpAddr>, String> {
    let listing = output("getent", &["ahosts", server])
        .map_err(|_| format!("{server} cannot be found on the network"))?;
    let local: Vec<IpAddr> = addresses_of(&listing)
        .into_iter()
        .filter(|address| is_local(*address))
        .collect();
    if local.is_empty() {
        return Err(format!("{server} is not on the local network"));
    }
    for (set, family) in [("nas_smb_servers", true), ("nas_smb_servers6", false)] {
        let elements: Vec<String> = local
            .iter()
            .filter(|address| address.is_ipv4() == family)
            .map(ToString::to_string)
            .collect();
        if elements.is_empty() {
            continue;
        }
        let elements = format!("{{ {} }}", elements.join(", "));
        output(
            "nft",
            &["add", "element", "inet", "nas_base", set, &elements],
        )
        .map_err(|_| {
            "the base firewall has no set for the network disks: update the NAS".to_owned()
        })?;
    }
    Ok(local)
}

/// What `mount -t cifs` is given: the share, the address the firewall opened and its owners.
pub(super) fn mount_arguments(
    server: &str,
    share: &str,
    address: IpAddr,
    credentials: &str,
    owners: (&str, &str),
    target: &Path,
) -> Vec<String> {
    vec![
        "-t".to_owned(),
        "cifs".to_owned(),
        format!("//{server}/{share}"),
        target.display().to_string(),
        "-o".to_owned(),
        format!(
            "credentials={credentials},ip={address},uid={},gid={},file_mode=0660,dir_mode=0770,\
             nosuid,nodev,noexec,soft,echo_interval=10",
            owners.0, owners.1
        ),
    ]
}

/// The lines of the credentials file of `mount.cifs`.
pub(super) fn credentials_of(username: &str, password: &str, domain: Option<&str>) -> String {
    let mut text = format!("username={username}\npassword={password}\n");
    if let Some(domain) = domain {
        text.push_str(&format!("domain={domain}\n"));
    }
    text
}

struct Account<'a> {
    server: &'a str,
    share: &'a str,
    username: &'a str,
    domain: Option<&'a str>,
}

/// Mounts the share on `target`, which it makes when missing.
fn mount(
    account: &Account,
    password: &str,
    target: &Path,
    owners: (&str, &str),
) -> Result<(), String> {
    let addresses = open_to(account.server)?;
    let credentials = Secret::write(
        "credentials",
        credentials_of(account.username, password, account.domain).as_bytes(),
    )?;
    fs::create_dir_all(target).map_err(|error| error.to_string())?;
    let arguments = mount_arguments(
        account.server,
        account.share,
        addresses[0],
        credentials.arg(),
        owners,
        target,
    );
    // A server that does not answer is given up on after a while, never waited for.
    let arguments: Vec<&str> = ["--kill-after=5", MOUNT_WITHIN, "mount"]
        .into_iter()
        .chain(arguments.iter().map(String::as_str))
        .collect();
    output_logged("timeout", &arguments, &log_path())
        .map(|_| ())
        .map_err(|reason| format!("{} cannot be connected: {reason}", account.share))
}

fn password_of(id: &str) -> Result<String, String> {
    let source = credential(id);
    let sealed = source.to_str().ok_or("invalid credential path")?;
    let bytes = raw_output(
        "systemd-creds",
        &[
            "decrypt",
            &format!("--name={}", credential_name(id)),
            sealed,
            "-",
        ],
    )?;
    String::from_utf8(bytes).map_err(|_| "unreadable password".to_owned())
}

fn save_password(id: &str, password: &str) -> Result<(), String> {
    let plain = Secret::write("password", password.as_bytes())?;
    let sealed = raw_output(
        "systemd-creds",
        &[
            "encrypt",
            &format!("--name={}", credential_name(id)),
            plain.arg(),
            "-",
        ],
    )?;
    let target = credential(id);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = target.with_extension("new");
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)
        .and_then(|mut file| file.write_all(&sealed))
        .map_err(|error| format!("cannot keep the password: {error}"))?;
    fs::rename(&temporary, &target).map_err(|error| error.to_string())
}

fn save_definitions(list: &[Definition]) -> Result<(), String> {
    let path = definitions_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = path.with_extension("json.new");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(list).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    fs::rename(&temporary, &path).map_err(|error| error.to_string())
}

fn definition(id: &str) -> Result<Definition, String> {
    definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .ok_or_else(|| "the network disk is not known".to_owned())
}

fn is_mounted(target: &Path) -> bool {
    output(
        "findmnt",
        &[
            "--mountpoint",
            &target.display().to_string(),
            "--noheadings",
            "--output",
            "FSTYPE",
        ],
    )
    .is_ok_and(|fstype| matches!(fstype.as_str(), "cifs" | "smb3"))
}

/// Mounts a known share with its password and the owners of the data disks.
async fn connect_definition(definition: &Definition) -> Result<(), String> {
    let (_, uid, gid) = owner()
        .await?
        .ok_or("the NAS has no user for its disks yet")?;
    let password = password_of(&definition.id)?;
    mount(
        &Account {
            server: &definition.server,
            share: &definition.share,
            username: &definition.username,
            domain: definition.domain.as_deref(),
        },
        &password,
        &target(&definition.id),
        (&uid, &gid),
    )
}

/// Unmounts a share after making sure nothing uses it.
fn disconnect_target(target: &Path) -> Result<(), String> {
    if !is_mounted(target) {
        return Ok(());
    }
    let mountpoint = target.display().to_string();
    let users = users_of(Path::new("/proc"), std::slice::from_ref(&mountpoint));
    if !users.is_empty() {
        return Err(format!(
            "the network disk is in use by {}",
            users.join(", ")
        ));
    }
    output_logged("umount", &[&mountpoint], &log_path()).map(|_| ())
}

fn new_id() -> Result<String, String> {
    let hex: String = random(6)?
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(format!("n-{hex}"))
}

async fn save(request: &NetworkRequest) -> Result<Answer, String> {
    let NetworkRequest::NetworkSave {
        id,
        name,
        server,
        share,
        username,
        domain,
        password,
        automatic,
    } = request
    else {
        return Err("not a save".to_owned());
    };
    let mut list = definitions();
    let id = match id {
        Some(id) => {
            definition(id)?;
            id.clone()
        }
        None => new_id()?,
    };
    match password {
        Some(password) => save_password(&id, password)?,
        None if !credential(&id).is_file() => {
            return Err("the network disk has no password".to_owned())
        }
        None => {}
    }
    let saved = Definition {
        id: id.clone(),
        name: name.clone(),
        server: server.clone(),
        share: share.clone(),
        username: username.clone(),
        domain: domain.clone(),
        automatic: *automatic,
    };
    match list.iter_mut().find(|known| known.id == id) {
        Some(known) => *known = saved,
        None => list.push(saved),
    }
    save_definitions(&list)?;
    reconcile().await?;
    Ok(Answer::default())
}

/// Mounts the share on a folder of the runtime directory, reads it and unmounts it.
fn test(request: &NetworkRequest) -> Result<Answer, String> {
    let NetworkRequest::NetworkTest {
        server,
        share,
        username,
        domain,
        password,
    } = request
    else {
        return Err("not a test".to_owned());
    };
    let probe = Secret::path("probe")?;
    let _ = fs::remove_dir(&probe);
    let mounted = mount(
        &Account {
            server,
            share,
            username,
            domain: domain.as_deref(),
        },
        password,
        &probe,
        ("0", "0"),
    );
    let read = mounted.and_then(|()| {
        fs::read_dir(&probe)
            .map(|_| ())
            .map_err(|error| format!("{share} cannot be read: {error}"))
    });
    let _ = output("umount", &[&probe.display().to_string()]);
    let _ = fs::remove_dir(&probe);
    read.map(|()| Answer::default())
}

async fn connect(id: &str) -> Result<Answer, String> {
    let known = definition(id)?;
    set_held(id, false)?;
    if !is_mounted(&target(id)) {
        connect_definition(&known).await?;
    }
    reconcile().await?;
    Ok(Answer::default())
}

async fn disconnect(id: &str) -> Result<Answer, String> {
    definition(id)?;
    // Held first: a reconciliation in between must not connect it again.
    set_held(id, true)?;
    disconnect_target(&target(id))?;
    reconcile().await?;
    Ok(Answer::default())
}

async fn forget(id: &str) -> Result<Answer, String> {
    definition(id)?;
    set_held(id, true)?;
    disconnect_target(&target(id))?;
    let list: Vec<Definition> = definitions()
        .into_iter()
        .filter(|known| known.id != id)
        .collect();
    save_definitions(&list)?;
    let _ = fs::remove_file(credential(id));
    set_held(id, false)?;
    reconcile().await?;
    Ok(Answer::default())
}

pub(in crate::services::storage) async fn run(request: &NetworkRequest) -> Result<Answer, String> {
    validate(request)?;
    match request {
        NetworkRequest::NetworkSave { .. } => save(request).await,
        NetworkRequest::NetworkTest { .. } => test(request),
        NetworkRequest::NetworkConnect { id } => connect(id).await,
        NetworkRequest::NetworkDisconnect { id } => disconnect(id).await,
        NetworkRequest::NetworkForget { id } => forget(id).await,
    }
}

/// The shares mounted now, granted to Explorer with the firewall kept open towards their servers,
/// and those that connect by themselves and the person did not disconnect, still to be mounted.
pub(in crate::services::storage) fn mounted_shares() -> (Vec<Grant>, Vec<Definition>) {
    let closed = held();
    let mut grants = Vec::new();
    let mut pending = Vec::new();
    for known in definitions() {
        let path = target(&known.id);
        if is_mounted(&path) {
            // A server that cannot be found now keeps its mount: the kernel reconnects by itself.
            if let Err(reason) = open_to(&known.server) {
                eprintln!("{}: {reason}", known.id);
            }
            grants.push(grant(&known));
        } else if known.automatic && !closed.contains(&known.id) {
            pending.push(known);
        }
    }
    (grants, pending)
}

fn grant(known: &Definition) -> Grant {
    Grant {
        id: known.id.clone(),
        name: known.name.clone(),
        path: target(&known.id),
        kind: "network",
    }
}

/// Mounts the shares still to be mounted, each within its time and all within `budget`: a server
/// that is off must not hold the reconciliation of the disks, which tries again in a minute.
pub(in crate::services::storage) fn connect_pending(
    pending: &[Definition],
    owners: (&str, &str),
    budget: std::time::Duration,
) -> Vec<Grant> {
    let started = std::time::Instant::now();
    let mut grants = Vec::new();
    for known in pending {
        if started.elapsed() >= budget {
            break;
        }
        let mounted = password_of(&known.id).and_then(|password| {
            mount(
                &Account {
                    server: &known.server,
                    share: &known.share,
                    username: &known.username,
                    domain: known.domain.as_deref(),
                },
                &password,
                &target(&known.id),
                owners,
            )
        });
        match mounted {
            Ok(()) => grants.push(grant(known)),
            Err(reason) => eprintln!("{} unavailable: {reason}", known.id),
        }
    }
    grants
}

#[cfg(test)]
#[path = "../../../../../tests/services/storage/network/root.rs"]
mod tests;
