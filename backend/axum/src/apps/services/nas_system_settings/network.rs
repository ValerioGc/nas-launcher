//! Network settings backed by [`machine`]; unreadable state returns an error.

mod machine;

use crate::{
    http::error::ApiFailure,
    services::notifications::{self, Notice, Tone},
    utils::{self, Mode},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// How the NAS is connected to the network.
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
    /// NetworkManager knows no Wi-Fi device: none is plugged in, or it has no driver
    NoDevice,
    /// The device did not give its list of networks
    ListFailed,
}

#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkState {
    pub connection: Connection,
    pub wifi: Vec<WifiNetwork>,
    pub wired_available: bool,
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

fn unreadable() -> ApiFailure {
    ApiFailure::unavailable("the network of this machine cannot be read")
}

pub async fn status(rescan: bool) -> Result<NetworkState, ApiFailure> {
    let interface = machine::read().await.ok_or_else(unreadable)?;
    let interface_name = interface.name.clone();
    let (found, wifi_problem) = match machine::scan(rescan).await {
        Ok(found) => (found, None),
        Err(problem) => (Vec::new(), Some(problem)),
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
    status(false).await
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
    status(false).await
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
    status(false).await
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
#[path = "../../../../../tests/apps/services/nas_system_settings/network.rs"]
mod tests;
