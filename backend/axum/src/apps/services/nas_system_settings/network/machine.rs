//! Reads the network through `ip`, sysfs and `nmcli`.
//! Mutation is gated by `NAS_NETWORK_MODE` in the app service.

use super::WifiProblem;
use serde_json::Value;
use std::{net::Ipv4Addr, time::Duration};
use tokio::process::Command;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Clone)]
pub struct Available {
    pub ssid: String,
    pub signal: u8,
    pub security: String,
    pub known: bool,
    pub active: bool,
}

/// A network card NetworkManager lists, wired or wireless, with its state (`connected`,
/// `disconnected`, `unavailable`, `unmanaged`, ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub name: String,
    pub wireless: bool,
    pub state: String,
}

#[derive(Debug, Clone)]
pub struct Interface {
    pub name: String,
    pub wireless: bool,
    pub ipv4: String,
    pub netmask: String,
    pub gateway: String,
    pub dns: Vec<String>,
    pub mac: String,
    pub speed: Option<String>,
    pub ssid: Option<String>,
    pub signal: Option<u8>,
    pub security: Option<String>,
}

async fn run(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().await.ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

async fn run_checked(program: &str, args: &[&str]) -> Result<(), String> {
    let call = Command::new(program).args(args).output();
    match tokio::time::timeout(CONNECT_TIMEOUT, call).await {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(output)) => Err(String::from_utf8_lossy(&output.stderr)
            .lines()
            .next()
            .unwrap_or("the command was refused")
            .trim()
            .to_owned()),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err(format!("{program} did not answer")),
    }
}

fn fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut escaped = false;
    for character in line.chars() {
        match character {
            '\\' if !escaped => escaped = true,
            ':' if !escaped => out.push(String::new()),
            _ => {
                escaped = false;
                if let Some(last) = out.last_mut() {
                    last.push(character);
                }
            }
        }
    }
    out
}

async fn device(kind: &str) -> Option<String> {
    run("nmcli", &["-t", "-f", "DEVICE,TYPE", "device"])
        .await?
        .lines()
        .map(fields)
        .find(|row| row.get(1).map(String::as_str) == Some(kind))
        .and_then(|row| row.first().cloned())
        .filter(|name| !name.is_empty())
}

async fn known_networks() -> Vec<String> {
    run("nmcli", &["-t", "-f", "NAME,TYPE", "connection", "show"])
        .await
        .unwrap_or_default()
        .lines()
        .map(fields)
        .filter(|row| row.get(1).map(String::as_str) == Some("802-11-wireless"))
        .filter_map(|row| row.first().cloned())
        .collect()
}

pub async fn scan(rescan: bool) -> Result<Vec<Available>, WifiProblem> {
    let Some(interface) = device("wifi").await else {
        return Err(
            if run("nmcli", &["-t", "general", "status"]).await.is_none() {
                WifiProblem::NoNetworkManager
            } else {
                WifiProblem::NoDevice
            },
        );
    };
    if rescan {
        let _ = run("nmcli", &["device", "wifi", "rescan", "ifname", &interface]).await;
    }
    let listed = run(
        "nmcli",
        &[
            "-t",
            "-f",
            "IN-USE,SSID,SIGNAL,SECURITY",
            "device",
            "wifi",
            "list",
            "ifname",
            &interface,
        ],
    )
    .await
    .ok_or(WifiProblem::ListFailed)?;
    let known = known_networks().await;

    let mut networks: Vec<Available> = Vec::new();
    for row in listed.lines().map(fields) {
        let [in_use, ssid, signal, security] = &row[..] else {
            continue;
        };
        if ssid.is_empty() {
            continue;
        }
        let found = Available {
            active: in_use.trim() == "*",
            signal: signal.trim().parse().unwrap_or(0),
            security: if security.trim().is_empty() {
                "open".to_owned()
            } else {
                security.trim().to_owned()
            },
            known: known.iter().any(|name| name == ssid),
            ssid: ssid.clone(),
        };
        match networks.iter_mut().find(|other| other.ssid == found.ssid) {
            Some(other) => {
                other.active |= found.active;
                other.known |= found.known;
                if found.signal > other.signal {
                    other.signal = found.signal;
                    other.security = found.security;
                }
            }
            None => networks.push(found),
        }
    }
    networks.sort_by_key(|network| std::cmp::Reverse(network.signal));
    Ok(networks)
}

pub async fn wired_available() -> bool {
    device("ethernet").await.is_some()
}

/// The wired and wireless cards NetworkManager lists, none when it does not answer.
pub async fn devices() -> Vec<Device> {
    run("nmcli", &["-t", "-f", "DEVICE,TYPE,STATE", "device"])
        .await
        .unwrap_or_default()
        .lines()
        .filter_map(|line| parse_device(&fields(line)))
        .collect()
}

fn parse_device(row: &[String]) -> Option<Device> {
    let [name, kind, state] = row else {
        return None;
    };
    let wireless = match kind.as_str() {
        "wifi" => true,
        "ethernet" => false,
        _ => return None,
    };
    (!name.is_empty()).then(|| Device {
        name: name.clone(),
        wireless,
        state: state.clone(),
    })
}

/// The card of the default route: the one the NAS is reached through.
pub async fn carrying() -> Option<String> {
    default_route().await.map(|(interface, _)| interface)
}

/// A cable is plugged into a wired card and that card has an address, whoever manages it
/// (NetworkManager or `/etc/network/interfaces`).
/// `NAS_NET_SYSFS` points it elsewhere, for the tests: never the cards of the host running them.
pub async fn wired_connected() -> bool {
    let root = std::env::var_os("NAS_NET_SYSFS").unwrap_or_else(|| "/sys/class/net".into());
    let Ok(mut entries) = tokio::fs::read_dir(root).await else {
        return false;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let physical = tokio::fs::metadata(path.join("device")).await.is_ok();
        let wireless = tokio::fs::metadata(path.join("wireless")).await.is_ok();
        let carrier = tokio::fs::read_to_string(path.join("carrier"))
            .await
            .is_ok_and(|value| value.trim() == "1");
        if !physical || wireless || !carrier {
            continue;
        }
        if address(&name).await.is_some() {
            return true;
        }
    }
    false
}

/// Brings a card back: it joins the network it knows, if there is one to join.
pub async fn connect_device(interface: &str) -> Result<(), String> {
    run_checked("nmcli", &["device", "connect", interface]).await
}

pub async fn join_wifi(ssid: &str, password: Option<&str>) -> Result<(), String> {
    let interface = device("wifi").await.ok_or("no wireless interface")?;
    let mut arguments = vec!["device", "wifi", "connect", ssid, "ifname", &interface];
    if let Some(secret) = password {
        arguments.extend(["password", secret]);
    }
    run_checked("nmcli", &arguments).await
}

pub async fn wifi_connection() -> Option<(String, Option<String>)> {
    let row = run(
        "nmcli",
        &["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device"],
    )
    .await?
    .lines()
    .map(fields)
    .find(|row| {
        row.get(1).map(String::as_str) == Some("wifi")
            && row.get(2).map(String::as_str) == Some("connected")
    })?;

    let interface = row.first().cloned().filter(|name| !name.is_empty())?;
    let ssid = row.get(3).cloned().filter(|name| !name.is_empty());
    Some((interface, ssid))
}

pub async fn leave(interface: &str) -> Result<(), String> {
    run_checked("nmcli", &["device", "disconnect", interface]).await
}

pub async fn join_wired() -> Result<(), String> {
    let interface = device("ethernet").await.ok_or("no wired interface")?;
    run_checked("nmcli", &["device", "connect", &interface]).await
}

async fn sysfs(interface: &str, file: &str) -> Option<String> {
    tokio::fs::read_to_string(format!("/sys/class/net/{interface}/{file}"))
        .await
        .ok()
        .map(|value| value.trim().to_owned())
}

async fn default_route() -> Option<(String, String)> {
    let routes: Value = serde_json::from_str(&run("ip", &["-json", "route"]).await?).ok()?;
    routes.as_array()?.iter().find_map(|route| {
        (route.get("dst")?.as_str()? == "default").then(|| {
            (
                route.get("dev")?.as_str()?.to_owned(),
                route
                    .get("gateway")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            )
                .into()
        })?
    })
}

/// First IPv4 address and netmask of the interface.
async fn address(interface: &str) -> Option<(String, String)> {
    let output = run("ip", &["-json", "addr", "show", "dev", interface]).await?;
    let parsed: Value = serde_json::from_str(&output).ok()?;
    let info = parsed.as_array()?.first()?.get("addr_info")?.as_array()?;
    let inet = info
        .iter()
        .find(|entry| entry.get("family").and_then(Value::as_str) == Some("inet"))?;
    let ipv4 = inet.get("local")?.as_str()?.to_owned();
    let prefix = inet.get("prefixlen")?.as_u64()? as u32;
    Some((ipv4, netmask(prefix)))
}

fn netmask(prefix: u32) -> String {
    let bits = u32::MAX.checked_shl(32 - prefix.min(32)).unwrap_or(0);
    Ipv4Addr::from(bits).to_string()
}

async fn dns(interface: &str) -> Vec<String> {
    let from_file: Vec<String> = tokio::fs::read_to_string("/etc/resolv.conf")
        .await
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.trim().strip_prefix("nameserver ").map(str::trim))
        .filter(|server| !server.starts_with("127."))
        .map(str::to_owned)
        .collect();
    if !from_file.is_empty() {
        return from_file;
    }
    run("resolvectl", &["dns", interface])
        .await
        .unwrap_or_default()
        .split_once(':')
        .map(|(_, servers)| {
            servers
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

async fn speed(interface: &str) -> Option<String> {
    let value: i64 = sysfs(interface, "speed").await?.parse().ok()?;
    if value <= 0 {
        return None;
    }
    Some(if value >= 1000 {
        format!("{} Gb/s", value / 1000)
    } else {
        format!("{value} Mb/s")
    })
}

fn property(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let parts = fields(line);
        parts
            .get(1)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty() && *value != "--")
            .map(str::to_owned)
    })
}

async fn ssid(interface: &str) -> Option<String> {
    let connection = property(
        &run(
            "nmcli",
            &["-t", "-f", "GENERAL.CONNECTION", "dev", "show", interface],
        )
        .await?,
    )?;
    let output = run(
        "nmcli",
        &[
            "-t",
            "-f",
            "802-11-wireless.ssid",
            "connection",
            "show",
            &connection,
        ],
    )
    .await;
    Some(output.as_deref().and_then(property).unwrap_or(connection))
}

async fn wifi_details(ssid: &str) -> (Option<u8>, Option<String>) {
    let list = run(
        "nmcli",
        &["-t", "-f", "SSID,SIGNAL,SECURITY", "dev", "wifi"],
    )
    .await
    .unwrap_or_default();
    parse_wifi_details(&list, ssid)
}

fn parse_wifi_details(list: &str, ssid: &str) -> (Option<u8>, Option<String>) {
    for row in list.lines().map(fields) {
        let [name, signal, security] = &row[..] else {
            continue;
        };
        if name == ssid {
            return (
                signal.trim().parse().ok(),
                (!security.trim().is_empty()).then(|| security.trim().to_owned()),
            );
        }
    }
    (None, None)
}

async fn kernel_signal(interface: &str) -> Option<u8> {
    let table = tokio::fs::read_to_string("/proc/net/wireless").await.ok()?;
    let line = table
        .lines()
        .find(|line| line.trim_start().starts_with(&format!("{interface}:")))?;
    let quality: f32 = line
        .split_whitespace()
        .nth(2)?
        .trim_end_matches('.')
        .parse()
        .ok()?;
    Some(((quality / 70.0) * 100.0).clamp(0.0, 100.0) as u8)
}

pub async fn read() -> Option<Interface> {
    let (name, gateway) = default_route().await?;
    let (ipv4, netmask) = address(&name).await?;
    let wireless = tokio::fs::metadata(format!("/sys/class/net/{name}/wireless"))
        .await
        .is_ok();
    let ssid = if wireless { ssid(&name).await } else { None };
    let (signal, security) = match ssid.as_deref() {
        Some(name) => wifi_details(name).await,
        None => (None, None),
    };
    let signal = match (wireless, signal) {
        (true, None) => kernel_signal(&name).await,
        (_, value) => value,
    };
    Some(Interface {
        speed: speed(&name).await,
        dns: dns(&name).await,
        mac: sysfs(&name, "address").await.unwrap_or_default(),
        name,
        wireless,
        ipv4,
        netmask,
        gateway,
        ssid,
        signal,
        security,
    })
}

#[cfg(test)]
#[path = "../../../../../../tests/apps/services/nas_system_settings/network/machine.rs"]
mod tests;
