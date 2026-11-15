//! Helpers shared by the launcher, the applications and the widgets.

use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
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

/// A path the tests can move with a variable, or where the installation puts it.
pub fn env_path(variable: &str, default: &str) -> PathBuf {
    std::env::var_os(variable).map_or_else(|| PathBuf::from(default), PathBuf::from)
}

/// The folder of the logs of the launcher and of its helpers.
pub fn log_dir() -> PathBuf {
    env_path("NAS_LOG_DIR", "/var/log/nas-os")
}

pub async fn systemctl(args: &[&str], timeout: Duration) -> Result<(), String> {
    systemctl_output(args, timeout).await.map(drop)
}

/// Runs systemctl and returns what it printed, or the first line of its error.
pub async fn systemctl_output(args: &[&str], timeout: Duration) -> Result<String, String> {
    let command = Command::new("systemctl").args(args).output();
    match tokio::time::timeout(timeout, command).await {
        Ok(Ok(output)) => answer(&output),
        Ok(Err(error)) => Err(format!("systemctl is not available: {error}")),
        Err(_) => Err(format!(
            "systemctl {} did not answer in time",
            args.join(" ")
        )),
    }
}

fn answer(output: &std::process::Output) -> Result<String, String> {
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(String::from_utf8_lossy(&output.stderr)
        .lines()
        .next()
        .unwrap_or("systemctl refused")
        .trim()
        .to_owned())
}

#[cfg(test)]
#[path = "../tests/utils.rs"]
mod tests;
