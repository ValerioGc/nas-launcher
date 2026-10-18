//! Systemd restart and shutdown with active-operation checks.
//! Privileged jobs block power actions to avoid interrupted system changes.
//! Power actions remain simulated unless `NAS_POWER_MODE=real`.

pub mod schedule;
pub mod wake;

use crate::{
    http::error::ApiFailure,
    services::notifications::{self, Notice, Tone},
    utils::{self, Mode},
    widgets::services::tasks,
};
use axum::http::StatusCode;
use serde::Serialize;
use sqlx::SqlitePool;
use std::time::Duration;
use tokio::process::Command;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Restart,
    Shutdown,
}
impl PowerAction {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "restart" => Some(Self::Restart),
            "shutdown" => Some(Self::Shutdown),
            _ => None,
        }
    }

    fn verb(self) -> &'static str {
        match self {
            Self::Restart => "reboot",
            Self::Shutdown => "poweroff",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Restart => "restart",
            Self::Shutdown => "shutdown",
        }
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct PowerOutcome {
    pub accepted: bool,
    pub simulated: bool,
}

/// Privileged update, package, and disk jobs that block restart and shutdown.
const ROOT_JOBS: [(&str, &str); 6] = [
    ("nas-os-update.service", "update"),
    ("nas-os-apt@", "apt"),
    ("nas-os-storage-initialize.service", "initialize"),
    ("nas-os-storage-rename.service", "rename"),
    ("nas-os-storage-eject.service", "eject"),
    ("nas-os-app-packages@", "packages"),
];

/// Application operations interrupted by shutdown and privileged jobs that block it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct Busy {
    pub tasks: Vec<String>,
    pub jobs: Vec<String>,
}

impl Busy {
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty() && self.jobs.is_empty()
    }
}

/// The jobs among the units `systemctl list-units` printed, once each, in the order of
/// `ROOT_JOBS`.
pub fn jobs_in(listed: &str) -> Vec<String> {
    let units: Vec<&str> = listed
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    ROOT_JOBS
        .iter()
        .filter(|(unit, _)| {
            units
                .iter()
                .any(|listed| listed == unit || (unit.ends_with('@') && listed.starts_with(unit)))
        })
        .map(|(_, job)| (*job).to_owned())
        .collect()
}

async fn running_jobs() -> Vec<String> {
    let patterns = ROOT_JOBS.map(|(unit, _)| {
        if unit.ends_with('@') {
            format!("{unit}*.service")
        } else {
            unit.to_owned()
        }
    });
    let command = Command::new("systemctl")
        .args(["list-units", "--plain", "--no-legend", "--all"])
        .arg("--state=activating,active,deactivating,reloading")
        .args(&patterns)
        .output();
    match tokio::time::timeout(COMMAND_TIMEOUT, command).await {
        Ok(Ok(output)) if output.status.success() => {
            jobs_in(&String::from_utf8_lossy(&output.stdout))
        }
        _ => Vec::new(),
    }
}

/// Application operations interrupted by shutdown and privileged jobs that block it.
pub async fn busy(db: &SqlitePool) -> Busy {
    Busy {
        tasks: tasks::list(db)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|task| task.state.active())
            .map(|task| task.name)
            .collect(),
        jobs: running_jobs().await,
    }
}

/// Restarts or shuts down the NAS when no privileged job is active.
pub async fn run(db: &SqlitePool, action: PowerAction) -> Result<PowerOutcome, ApiFailure> {
    let jobs = running_jobs().await;
    if !jobs.is_empty() {
        return Err(ApiFailure(
            StatusCode::CONFLICT,
            format!("the NAS is busy: {}", jobs.join(", ")),
        ));
    }
    if !utils::is_real(Mode::Power) {
        notifications::record(
            db,
            "system",
            Tone::Info,
            Notice::new(
                "powerSimulated",
                "System action simulated",
                format!(
                    "System {} simulated, the system was not changed.",
                    action.id()
                ),
            )
            .with("action", action.id()),
        )
        .await;
        return Ok(PowerOutcome {
            accepted: true,
            simulated: true,
        });
    }

    let command = Command::new("systemctl").arg(action.verb()).output();
    match tokio::time::timeout(COMMAND_TIMEOUT, command).await {
        Ok(Ok(output)) if output.status.success() => {}
        Ok(Ok(output)) => {
            let detail = String::from_utf8_lossy(&output.stderr);
            let message = detail.lines().next().unwrap_or("systemctl refused").trim();
            notifications::record(
                db,
                "system",
                Tone::Error,
                Notice::new(
                    "powerRefused",
                    "System action refused",
                    format!("System {} refused ({message}).", action.id()),
                )
                .with("action", action.id())
                .with("reason", message)
                .detail("power", &detail, None)
                .await,
            )
            .await;
            return Err(ApiFailure::unavailable(format!(
                "{} refused: {message}",
                action.id()
            )));
        }

        Ok(Err(error)) => {
            return Err(ApiFailure::unavailable(format!(
                "{} failed: {error}",
                action.id()
            )));
        }

        Err(_) => {}
    }

    Ok(PowerOutcome {
        accepted: true,
        simulated: false,
    })
}

#[cfg(test)]
#[path = "../../../../tests/apps/services/nas_system_settings/power.rs"]
mod tests;
