//! Helpers shared by the launcher, the applications and the widgets.

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Apps,
    Apt,
    Encryption,
    Firewall,
    Hostname,
    Network,
    NetworkDisks,
    Packages,
    Power,
    Processes,
    Reset,
    Storage,
    Terminal,
    Token,
}

impl Mode {
    pub fn variable(self) -> &'static str {
        match self {
            Self::Apps => "NAS_APPS_MODE",
            Self::Apt => "NAS_APT_MODE",
            Self::Encryption => "NAS_ENCRYPTION_MODE",
            Self::Firewall => "NAS_FIREWALL_MODE",
            Self::Hostname => "NAS_HOSTNAME_MODE",
            Self::Network => "NAS_NETWORK_MODE",
            Self::NetworkDisks => "NAS_NETWORK_DISKS_MODE",
            Self::Packages => "NAS_PACKAGES_MODE",
            Self::Power => "NAS_POWER_MODE",
            Self::Processes => "NAS_PROCESSES_MODE",
            Self::Reset => "NAS_RESET_MODE",
            Self::Storage => "NAS_STORAGE_MODE",
            Self::Terminal => "NAS_TERMINAL_MODE",
            Self::Token => "NAS_TOKEN_MODE",
        }
    }
}

/// True only when the switch of that mode is `real`.
pub fn is_real(mode: Mode) -> bool {
    says_real(std::env::var(mode.variable()).ok().as_deref())
}

fn says_real(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.eq_ignore_ascii_case("real"))
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

pub async fn systemctl(args: &[&str], timeout: Duration) -> Result<(), String> {
    let command = Command::new("systemctl").args(args).output();
    match tokio::time::timeout(timeout, command).await {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(output)) => Err(String::from_utf8_lossy(&output.stderr)
            .lines()
            .next()
            .unwrap_or("systemctl refused")
            .trim()
            .to_owned()),
        Ok(Err(error)) => Err(format!("systemctl is not available: {error}")),
        Err(_) => Err(format!(
            "systemctl {} did not answer in time",
            args.join(" ")
        )),
    }
}

#[cfg(test)]
#[path = "../../tests/utils.rs"]
mod tests;
