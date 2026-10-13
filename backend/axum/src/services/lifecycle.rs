//! Controls installed apps through `nas-app@<id>.service`.
//! The system helper reads `run.exec` from the app manifest

use crate::{
    http::error::ApiFailure,
    services::{
        manifest,
        notifications::{self, Notice, Tone},
    },
    utils::{self, Mode},
};
use std::{collections::HashMap, path::Path, time::Duration};
use tokio::{net::UnixStream, process::Command};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
const READY_TIMEOUT: Duration = Duration::from_secs(8);
const READY_INTERVAL: Duration = Duration::from_millis(100);

pub fn unit_of(id: &str) -> Option<String> {
    manifest::is_identifier(id).then(|| format!("nas-app@{id}.service"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Start,
    Stop,
    Restart,
}

impl Action {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "start" => Some(Self::Start),
            "stop" => Some(Self::Stop),
            "restart" => Some(Self::Restart),
            _ => None,
        }
    }

    pub fn verb(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
        }
    }
}

pub async fn running(ids: &[String]) -> HashMap<String, bool> {
    let units: Vec<String> = ids.iter().filter_map(|id| unit_of(id)).collect();
    if !utils::is_real(Mode::Apps) || units.is_empty() {
        return ids.iter().map(|id| (id.clone(), false)).collect();
    }

    let output = Command::new("systemctl")
        .arg("is-active")
        .args(&units)
        .output();
    let Ok(Ok(output)) = tokio::time::timeout(COMMAND_TIMEOUT, output).await else {
        return ids.iter().map(|id| (id.clone(), false)).collect();
    };

    let answer = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut states = answer.lines().map(|line| line.trim() == "active");
    ids.iter()
        .map(|id| {
            let active = unit_of(id).is_some() && states.next().unwrap_or(false);
            (id.clone(), active)
        })
        .collect()
}

pub async fn is_running(id: &str) -> bool {
    running(&[id.to_owned()])
        .await
        .get(id)
        .copied()
        .unwrap_or(false)
}

async fn wait_for_socket(path: &Path, timeout: Duration) -> bool {
    tokio::time::timeout(timeout, async {
        loop {
            if UnixStream::connect(path).await.is_ok() {
                return;
            }
            tokio::time::sleep(READY_INTERVAL).await;
        }
    })
    .await
    .is_ok()
}

fn stopped_state(state: &str) -> bool {
    matches!(state, "inactive" | "failed")
}

pub(crate) async fn wait_for_stop(unit: &str) -> Result<(), String> {
    let stopped = tokio::time::timeout(READY_TIMEOUT, async {
        loop {
            let output = Command::new("systemctl")
                .args(["is-active", unit])
                .output()
                .await
                .map_err(|error| format!("systemctl is-active {unit}: {error}"))?;
            let state = String::from_utf8_lossy(&output.stdout);
            if stopped_state(state.trim()) {
                return Ok(());
            }
            if state.trim().is_empty() {
                return Err(format!(
                    "systemctl is-active {unit}: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            tokio::time::sleep(READY_INTERVAL).await;
        }
    })
    .await;
    stopped.map_err(|_| format!("{unit} is still running after stop"))?
}

pub async fn run(id: &str, action: Action) -> Result<(), ApiFailure> {
    let Some(unit) = unit_of(id) else {
        return Err(ApiFailure::bad(format!(
            "not an application identifier: {id}"
        )));
    };
    // An application that answers is started already: nothing to ask systemd, and the browser
    // never asks the page of a stopped one to find out.
    if action == Action::Start {
        if let Some(app) = manifest::read(id).await {
            if UnixStream::connect(&app.socket).await.is_ok() {
                return Ok(());
            }
        }
    }
    if !utils::is_real(Mode::Apps) {
        return Err(ApiFailure::unavailable(
            "Starting applications is not available on this installation.",
        ));
    }

    utils::systemctl(&[action.verb(), &unit], COMMAND_TIMEOUT)
        .await
        .map_err(|reason| {
            ApiFailure::unavailable(format!(
                "systemctl could not {} {unit}: {reason}",
                action.verb()
            ))
        })?;

    if action == Action::Stop {
        wait_for_stop(&unit)
            .await
            .map_err(ApiFailure::unavailable)?;
    } else {
        let app = manifest::read(id)
            .await
            .ok_or_else(|| ApiFailure::missing(format!("unknown application: {id}")))?;
        if !wait_for_socket(&app.socket, READY_TIMEOUT).await {
            return Err(ApiFailure::unavailable(format!(
                "{id} did not become ready after {}",
                action.verb()
            )));
        }
    }
    Ok(())
}

pub async fn restart_after_install(db: &sqlx::SqlitePool, id: &str, notify: bool) {
    if !utils::is_real(Mode::Apps) {
        return;
    }
    if let Err(failure) = run(id, Action::Restart).await {
        eprintln!("{id} was installed but did not start: {}", failure.1);
        if !notify {
            return;
        }
        notifications::record(
            db,
            "nas-app-manager",
            Tone::Warning,
            Notice::new(
                "appNotStarted",
                "Application not started",
                format!("{id} installed, its process did not start ({}).", failure.1),
            )
            .with("app", id)
            .with("reason", failure.1.clone())
            .detail(
                &format!("app-{id}"),
                &failure.1,
                Some(&format!("nas-app@{id}.service")),
            )
            .await,
        )
        .await;
    }
}

pub async fn stop_before_removal(id: &str) {
    if !utils::is_real(Mode::Apps) {
        return;
    }
    if let Err(failure) = run(id, Action::Stop).await {
        eprintln!("{id} was removed while running: {}", failure.1);
    }
}

/// Deletes the state directory of a stopped application.
pub async fn remove_data(id: &str) {
    let Some(unit) = unit_of(id) else {
        return;
    };
    if !utils::is_real(Mode::Apps) {
        return;
    }
    if let Err(reason) = utils::systemctl(&["clean", "--what=state", &unit], COMMAND_TIMEOUT).await
    {
        eprintln!("the data of {id} were kept: {reason}");
    }
}

#[cfg(test)]
#[path = "../../../tests/services/lifecycle.rs"]
mod tests;
