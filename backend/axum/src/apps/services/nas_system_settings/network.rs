//! Network settings backed by [`machine`]. Unreadable state returns an error.
//!
//! Wired connectivity takes precedence over Wi-Fi. An adapter can be disabled only
//! when another connection remains available. Reconciliation preserves disabled
//! state but reactivates an adapter if connectivity would otherwise be lost.

mod machine;

use crate::{
    http::error::ApiFailure,
    services::{
        notifications::{self, Notice, Tone},
        settings::{self, Section},
    },
    utils::{self, Mode},
};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::time::Duration;

/// Interval for reconciling disabled adapters while another connection is available.
const KEEP_DISABLED_EVERY: Duration = Duration::from_secs(30);

/// Current NAS network connectivity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    Lan,
    Wifi,
}

#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub kind: LinkKind,
    pub connected: bool,
    pub interface: String,
    pub ssid: Option<String>,
    pub signal: Option<u8>,
    pub security: Option<String>,
    pub ipv4: String,
    pub netmask: String,
    pub gateway: String,
    pub dns: Vec<String>,
    pub mac: String,
    pub speed: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal: u8,
    pub security: String,
    pub known: bool,
}

/// Why no wireless network could be listed, for the page to say so instead of "none found".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum WifiProblem {
    /// `nmcli` is missing or NetworkManager is not running
    NoNetworkManager,
    /// No Wi-Fi device is available to NetworkManager.
    NoDevice,
    /// The device did not give its list of networks
    ListFailed,
    /// The Wi-Fi card was turned off from the page
    Disabled,
}

/// Adapter activity reported by NetworkManager.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AdapterState {
    Connected,
    Disconnected,
    /// No cable, or no radio
    Unavailable,
    /// The adapter is managed by `/etc/network/interfaces` rather than NetworkManager.
    Unmanaged,
}

impl AdapterState {
    fn from_nmcli(state: &str) -> Self {
        match state {
            "connected" => Self::Connected,
            "unavailable" => Self::Unavailable,
            "unmanaged" => Self::Unmanaged,
            _ => Self::Disconnected,
        }
    }
}

/// A network card of the NAS and whether it can be turned off or on from the page.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Adapter {
    pub interface: String,
    pub kind: LinkKind,
    pub state: AdapterState,
    pub enabled: bool,
    /// It carries the network the NAS is reached through
    pub in_use: bool,
    pub can_toggle: bool,
}

#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkState {
    pub connection: Connection,
    pub wifi: Vec<WifiNetwork>,
    pub wired_available: bool,
    /// An addressed wired connection takes precedence over Wi-Fi.
    pub wired_connected: bool,
    pub adapters: Vec<Adapter>,
    pub can_connect: bool,
    pub can_disconnect: bool,
    pub wifi_problem: Option<WifiProblem>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WifiRequest {
    pub ssid: String,
    pub password: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdapterRequest {
    pub interface: String,
    pub enabled: bool,
}

/// The cards turned off from the page, by interface name.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NetworkSettings {
    pub disabled: Vec<String>,
}

async fn disabled(db: &SqlitePool) -> Vec<String> {
    settings::read::<NetworkSettings>(db, Section::Network)
        .await
        .map(|saved| saved.disabled)
        .unwrap_or_default()
}

/// Adapter state including launcher-managed disable flags.
fn adapters(devices: &[machine::Device], carrying: &str, off: &[String]) -> Vec<Adapter> {
    let editable = utils::is_real(Mode::Network);
    devices
        .iter()
        .map(|device| {
            let state = AdapterState::from_nmcli(&device.state);
            let enabled = !off.contains(&device.name);
            let in_use = device.name == carrying;
            Adapter {
                interface: device.name.clone(),
                kind: if device.wireless {
                    LinkKind::Wifi
                } else {
                    LinkKind::Lan
                },
                state,
                enabled,
                in_use,
                can_toggle: editable && state != AdapterState::Unmanaged && (!enabled || !in_use),
            }
        })
        .collect()
}

fn unreadable() -> ApiFailure {
    ApiFailure::unavailable("the network of this machine cannot be read")
}

pub async fn status(db: &SqlitePool, rescan: bool) -> Result<NetworkState, ApiFailure> {
    let interface = machine::read().await.ok_or_else(unreadable)?;
    let interface_name = interface.name.clone();
    let off = disabled(db).await;
    let devices = machine::devices().await;
    let wifi_off = devices
        .iter()
        .any(|device| device.wireless && off.contains(&device.name));
    // Skips network discovery on disabled adapters.
    let (found, wifi_problem) = if wifi_off {
        (Vec::new(), Some(WifiProblem::Disabled))
    } else {
        match machine::scan(rescan).await {
            Ok(found) => (found, None),
            Err(problem) => (Vec::new(), Some(problem)),
        }
    };
    let wifi = found
        .into_iter()
        .map(|found| WifiNetwork {
            ssid: found.ssid,
            signal: found.signal,
            security: found.security,
            known: found.known,
        })
        .collect();

    Ok(NetworkState {
        connection: Connection {
            kind: if interface.wireless {
                LinkKind::Wifi
            } else {
                LinkKind::Lan
            },
            connected: true,
            interface: interface.name,
            ssid: interface.ssid,
            signal: interface.signal,
            security: interface.security,
            ipv4: interface.ipv4,
            netmask: interface.netmask,
            gateway: interface.gateway,
            dns: interface.dns,
            mac: interface.mac,
            speed: interface.speed,
        },
        wifi,
        wired_available: machine::wired_available().await,
        wired_connected: machine::wired_connected().await,
        adapters: adapters(&devices, &interface_name, &off),
        can_connect: utils::is_real(Mode::Network),
        can_disconnect: utils::is_real(Mode::Network) && leavable(&interface_name).await.is_some(),
        wifi_problem,
    })
}

async fn leavable(carrying: &str) -> Option<String> {
    let (interface, _) = machine::wifi_connection().await?;
    (interface != carrying).then_some(interface)
}

fn refuse_disabled() -> ApiFailure {
    ApiFailure::unavailable("Network changes are not available on this installation.")
}

fn from_nmcli(reason: String) -> ApiFailure {
    if reason.contains("Secrets were required") || reason.contains("secrets") {
        ApiFailure::bad("the network refused the password")
    } else {
        ApiFailure::unavailable(reason)
    }
}

pub async fn connect_wifi(
    db: &SqlitePool,
    request: WifiRequest,
) -> Result<NetworkState, ApiFailure> {
    if machine::read().await.is_none() {
        return Err(unreadable());
    }
    if !utils::is_real(Mode::Network) {
        return Err(refuse_disabled());
    }

    let ssid = request.ssid.trim();
    if ssid.is_empty() || ssid.len() > 32 {
        return Err(ApiFailure::bad("invalid network name"));
    }
    // Preserves wired connectivity instead of joining Wi-Fi.
    if machine::wired_connected().await {
        return Err(ApiFailure(
            StatusCode::CONFLICT,
            "the network cable is connected: the NAS does not join a wireless network too"
                .to_owned(),
        ));
    }
    let off = disabled(db).await;
    if machine::devices()
        .await
        .iter()
        .any(|device| device.wireless && disabled_now(&device.name, &off))
    {
        return Err(ApiFailure::bad("the Wi-Fi card is turned off"));
    }

    let password = request
        .password
        .as_deref()
        .map(str::trim)
        .filter(|secret| !secret.is_empty());
    if let Some(secret) = password {
        if !(8..=64).contains(&secret.chars().count()) {
            return Err(ApiFailure::bad("the password of this network is too short"));
        }
    }

    if let Err(reason) = machine::join_wifi(ssid, password).await {
        return Err(refused(db, reason).await);
    }

    notifications::record(
        db,
        "nas-system-settings",
        Tone::Success,
        Notice::new(
            "networkWireless",
            "Network changed",
            format!("NAS connected to the wireless network {ssid}."),
        )
        .with("network", ssid),
    )
    .await;
    status(db, false).await
}

pub async fn connect_wired(db: &SqlitePool) -> Result<NetworkState, ApiFailure> {
    if machine::read().await.is_none() {
        return Err(unreadable());
    }
    if !utils::is_real(Mode::Network) {
        return Err(refuse_disabled());
    }
    if !machine::wired_available().await {
        return Err(ApiFailure::bad("no wired interface available"));
    }
    let off = disabled(db).await;
    if machine::devices()
        .await
        .iter()
        .any(|device| !device.wireless && disabled_now(&device.name, &off))
    {
        return Err(ApiFailure::bad("the wired card is turned off"));
    }

    if let Err(reason) = machine::join_wired().await {
        return Err(refused(db, reason).await);
    }
    notifications::record(
        db,
        "nas-system-settings",
        Tone::Success,
        Notice::new(
            "networkWired",
            "Network changed",
            "NAS connected to the wired network.",
        ),
    )
    .await;
    status(db, false).await
}

pub async fn disconnect_wifi(db: &SqlitePool) -> Result<NetworkState, ApiFailure> {
    let carrying = machine::read().await.ok_or_else(unreadable)?.name;
    if !utils::is_real(Mode::Network) {
        return Err(refuse_disabled());
    }

    let Some((interface, ssid)) = machine::wifi_connection().await else {
        return Err(ApiFailure::bad("there is no wireless connection to leave"));
    };
    if interface == carrying {
        return Err(ApiFailure::bad(
            "this is the connection the launcher is reached through: it cannot be left from here",
        ));
    }

    if let Err(reason) = machine::leave(&interface).await {
        return Err(refused(db, reason).await);
    }
    let network = ssid.unwrap_or(interface);
    notifications::record(
        db,
        "nas-system-settings",
        Tone::Info,
        Notice::new(
            "networkLeft",
            "Network disconnected",
            format!("NAS disconnected from the wireless network {network}."),
        )
        .with("network", network),
    )
    .await;
    status(db, false).await
}

fn disabled_now(interface: &str, off: &[String]) -> bool {
    off.iter().any(|name| name == interface)
}

/// Changes adapter enable state while protecting the active NAS connection.
pub async fn set_adapter(
    db: &SqlitePool,
    request: AdapterRequest,
) -> Result<NetworkState, ApiFailure> {
    let carrying = machine::read().await.ok_or_else(unreadable)?.name;
    if !utils::is_real(Mode::Network) {
        return Err(refuse_disabled());
    }
    let devices = machine::devices().await;
    let Some(device) = devices
        .iter()
        .find(|device| device.name == request.interface)
    else {
        return Err(ApiFailure::bad("there is no such network card"));
    };
    if AdapterState::from_nmcli(&device.state) == AdapterState::Unmanaged {
        return Err(ApiFailure::bad(
            "this card is not managed by NetworkManager: it cannot be turned off from here",
        ));
    }

    let mut off = disabled(db).await;
    off.retain(|name| name != &device.name);
    if request.enabled {
        settings::write(db, Section::Network, &NetworkSettings { disabled: off }).await?;
        // Enabling an adapter succeeds even when no known network is currently available.
        let _ = machine::connect_device(&device.name).await;
    } else {
        if device.name == carrying {
            return Err(ApiFailure::bad(
                "this is the card the NAS is reached through: it cannot be turned off from here",
            ));
        }
        off.push(device.name.clone());
        settings::write(db, Section::Network, &NetworkSettings { disabled: off }).await?;
        if AdapterState::from_nmcli(&device.state) == AdapterState::Connected {
            if let Err(reason) = machine::leave(&device.name).await {
                return Err(refused(db, reason).await);
            }
        }
    }

    let (key, title, message) = if request.enabled {
        ("networkAdapterOn", "Network card on", "turned on")
    } else {
        ("networkAdapterOff", "Network card off", "turned off")
    };
    notifications::record(
        db,
        "nas-system-settings",
        Tone::Info,
        Notice::new(
            key,
            title,
            format!("Network card {} {message}.", device.name),
        )
        .with("interface", device.name.clone()),
    )
    .await;
    status(db, false).await
}

/// Calculates whether disabled-adapter reconciliation should disconnect, reactivate, or
/// preserve an adapter.
fn keep_off(device: &machine::Device, carrying: Option<&str>) -> Option<bool> {
    let connected = AdapterState::from_nmcli(&device.state) == AdapterState::Connected;
    match carrying {
        // Reactivates the adapter when no other connection remains available.
        None => (!connected).then_some(true),
        Some(route) if route != device.name && connected => Some(false),
        _ => None,
    }
}

/// Keeps the cards turned off from the page off while another card carries the network, and
/// brings one back when the NAS would otherwise be unreachable. Only where changes are real.
pub async fn keep_disabled(db: SqlitePool) {
    if !utils::is_real(Mode::Network) {
        return;
    }
    loop {
        let off = disabled(&db).await;
        if !off.is_empty() {
            let carrying = machine::carrying().await;
            for device in machine::devices().await {
                if !disabled_now(&device.name, &off) {
                    continue;
                }
                let _ = match keep_off(&device, carrying.as_deref()) {
                    Some(true) => machine::connect_device(&device.name).await,
                    Some(false) => machine::leave(&device.name).await,
                    None => Ok(()),
                };
            }
        }
        tokio::time::sleep(KEEP_DISABLED_EVERY).await;
    }
}

async fn refused(db: &SqlitePool, reason: String) -> ApiFailure {
    notifications::record(
        db,
        "nas-system-settings",
        Tone::Error,
        Notice::new(
            "networkFailed",
            "Network not changed",
            format!("Network connection unchanged ({reason})."),
        )
        .with("reason", reason.clone())
        .detail("network", &reason, Some("NetworkManager.service"))
        .await,
    )
    .await;
    from_nmcli(reason)
}

#[cfg(test)]
#[path = "../../../../tests/apps/services/nas_system_settings/network.rs"]
mod tests;
