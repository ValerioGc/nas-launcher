//! Debian package updates through privileged systemd helpers.
//! The launcher reads available updates without privileges. Refresh and upgrade jobs
//! run sequentially in the background when `NAS_APT_MODE=real`.

use crate::{
    http::error::ApiFailure,
    services::{
        database,
        notifications::{self, Notice, Tone},
        session::Session,
    },
    utils::{self, Mode},
    widgets::services::tasks::{self, NewTask, TaskState, TaskUpdate},
};
use axum::http::StatusCode;
use serde::Serialize;
use sqlx::SqlitePool;
use std::time::Duration;
use tokio::process::Command;

const LIST_TIMEOUT: Duration = Duration::from_secs(60);
const JOB_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const MAX_SELECTION: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct Package {
    pub id: String,
    pub current: String,
    pub version: String,
    pub level: Level,
}

/// Classifies Debian update severity from security, release, updates, and other repositories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Other,
    Stable,
    Security,
}

impl Level {
    pub fn id(self) -> &'static str {
        match self {
            Self::Other => "other",
            Self::Stable => "stable",
            Self::Security => "security",
        }
    }

    /// The level of the archives an upgrade is listed in, `trixie-security,trixie-updates`.
    pub fn of(archives: &str) -> Self {
        archives
            .split(',')
            .map(|archive| match archive.split_once('-') {
                Some((_, "security")) => Self::Security,
                None | Some((_, "updates" | "proposed-updates")) => Self::Stable,
                Some(_) => Self::Other,
            })
            .max()
            .unwrap_or(Self::Other)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Job {
    Refresh,
    Upgrade,
}
impl Job {
    fn id(self) -> &'static str {
        match self {
            Self::Refresh => "refresh",
            Self::Upgrade => "upgrade",
        }
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SystemUpdates {
    pub packages: Vec<Package>,
    pub running: Option<Job>,
    pub manageable: bool,
}

pub fn is_package_name(name: &str) -> bool {
    let mut chars = name.chars();
    name.len() <= 128
        && chars
            .next()
            .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "+-.".contains(c))
}

pub fn parse(output: &str) -> Vec<Package> {
    output
        .lines()
        .filter_map(|line| {
            let (head, from) = line.split_once(" [upgradable from: ")?;
            let mut fields = head.split_whitespace();
            let (id, archives) = fields.next()?.split_once('/')?;
            let version = fields.next()?;
            is_package_name(id).then(|| Package {
                id: id.to_owned(),
                current: from.trim_end_matches(']').trim().to_owned(),
                version: version.to_owned(),
                level: Level::of(archives),
            })
        })
        .collect()
}

pub async fn upgradable() -> Result<Vec<Package>, ApiFailure> {
    let command = Command::new("apt")
        .args(["list", "--upgradable"])
        .env("LC_ALL", "C")
        .output();
    match tokio::time::timeout(LIST_TIMEOUT, command).await {
        Ok(Ok(output)) if output.status.success() => {
            Ok(parse(&String::from_utf8_lossy(&output.stdout)))
        }
        Ok(Ok(_)) => Err(ApiFailure::unavailable("apt could not list the upgrades")),
        Ok(Err(_)) => Err(ApiFailure::unavailable(
            "apt is not available on this machine",
        )),
        Err(_) => Err(ApiFailure::unavailable("apt did not answer in time")),
    }
}

pub async fn state(session: &Session) -> Result<SystemUpdates, ApiFailure> {
    Ok(SystemUpdates {
        packages: upgradable().await?,
        running: session.0.read().await.apps.apt_job,
        manageable: utils::is_real(Mode::Apt),
    })
}

pub async fn start(
    db: &SqlitePool,
    session: &Session,
    job: Job,
    ids: &[String],
) -> Result<SystemUpdates, ApiFailure> {
    if !utils::is_real(Mode::Apt) {
        return Err(ApiFailure::unavailable(
            "System updates cannot be installed on this installation.",
        ));
    }
    if job == Job::Upgrade {
        select(ids).await?;
    }
    {
        let mut state = session.0.write().await;
        if state.apps.apt_job.is_some() {
            return Err(ApiFailure(
                StatusCode::CONFLICT,
                "a system update is already running".into(),
            ));
        }
        state.apps.apt_job = Some(job);
    }

    let task = format!("apt-{}", job.id());
    let registered = tasks::register(
        db,
        NewTask {
            id: task.clone(),
            source: "nas-system-settings".into(),
            name: match job {
                Job::Refresh => "Package index".into(),
                Job::Upgrade => format!("{} system packages", ids.len()),
            },
            detail: String::new(),
            progress: None,
            pausable: false,
            cancellable: false,
        },
    )
    .await;
    if let Err(failure) = registered {
        session.0.write().await.apps.apt_job = None;
        return Err(failure);
    }

    let (pool, running, count) = (db.clone(), session.clone(), ids.len());
    tokio::spawn(async move {
        let unit = format!("nas-os-apt@{}.service", job.id());
        let outcome = utils::systemctl(&["start", &unit], JOB_TIMEOUT).await;
        finish(&pool, &task, job, count, outcome).await;
        running.0.write().await.apps.apt_job = None;
    });

    state(session).await
}

async fn select(ids: &[String]) -> Result<(), ApiFailure> {
    if ids.is_empty() || ids.len() > MAX_SELECTION {
        return Err(ApiFailure::bad("invalid update selection"));
    }

    let offered = upgradable().await?;
    if let Some(unknown) = ids
        .iter()
        .find(|id| !offered.iter().any(|package| &package.id == *id))
    {
        return Err(ApiFailure::bad(format!(
            "no upgrade available for {unknown}"
        )));
    }

    let path = database::beside("apt-upgrade")
        .ok_or_else(|| ApiFailure::unavailable("no state directory for the selection"))?;
    tokio::fs::write(&path, ids.join("\n"))
        .await
        .map_err(|error| ApiFailure::unavailable(format!("cannot write the selection: {error}")))
}

async fn finish(db: &SqlitePool, task: &str, job: Job, count: usize, outcome: Result<(), String>) {
    let _ = tasks::update(
        db,
        task,
        TaskUpdate {
            detail: outcome.as_ref().err().cloned(),
            progress: Some(outcome.is_ok().then_some(100)),
            state: Some(if outcome.is_ok() {
                TaskState::Done
            } else {
                TaskState::Failed
            }),
            ..TaskUpdate::default()
        },
    )
    .await;
    if outcome.is_ok() && job == Job::Upgrade {
        notifications::record(
            db,
            "nas-system-settings",
            Tone::Success,
            Notice::new(
                "systemPackagesUpdated",
                "System packages updated",
                format!("{count} system packages updated."),
            )
            .with("count", count.to_string()),
        )
        .await;
    }
}

#[cfg(test)]
#[path = "../../../../tests/apps/services/nas_system_settings/system_updates.rs"]
mod tests;
