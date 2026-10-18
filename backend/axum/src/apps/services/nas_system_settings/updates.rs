//! Coordinated updates for the wizard, gateway, and launcher.
//!
//! Downloads available component releases and verifies their published checksums.
//! The privileged update unit installs them in wizard, gateway, launcher order.
//! A shared backup supports rollback if installation or health checks fail.

use crate::{
    http::error::ApiFailure,
    services::{
        database, mail_events,
        notifications::{self, Notice, Tone},
        packages::{catalogue::Provider, credentials, forge},
        session::SessionData,
    },
    utils::{self, Mode},
    widgets::services::tasks::{self, NewTask, TaskState, TaskUpdate},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{collections::BTreeMap, path::PathBuf, time::Duration};
use tokio::process::Command;

/// The unit that installs the downloaded releases and restarts what they replaced, installed by
/// `system/packages/ui_installation/`.
const UPDATE_UNIT: &str = "nas-os-update.service";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// Independently versioned NAS component, ordered by wizard, gateway, and launcher
/// installation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Component {
    Wizard,
    Gateway,
    Launcher,
}

impl Component {
    /// The order the settings show them in.
    pub const SHOWN: [Self; 3] = [Self::Launcher, Self::Gateway, Self::Wizard];

    pub fn id(self) -> &'static str {
        match self {
            Self::Wizard => "wizard",
            Self::Gateway => "gateway",
            Self::Launcher => "launcher",
        }
    }

    /// Configured repository or a sibling repository under the launcher owner.
    pub fn repository(self) -> String {
        let launcher = std::env::var("NAS_LAUNCHER_REPOSITORY").unwrap_or_default();
        let (variable, name) = match self {
            Self::Launcher => return launcher,
            Self::Gateway => ("NAS_GATEWAY_REPOSITORY", "nas-gateway"),
            Self::Wizard => ("NAS_WIZARD_REPOSITORY", "nas-wizard"),
        };
        std::env::var(variable)
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| sibling(&launcher, name))
    }

    fn asset(self) -> String {
        let (variable, default) = match self {
            Self::Launcher => ("NAS_LAUNCHER_ASSET", "nas-os-{version}-linux-x64.tar.gz"),
            Self::Gateway => (
                "NAS_GATEWAY_ASSET",
                "nas-gateway-{version}-linux-x64.tar.gz",
            ),
            Self::Wizard => ("NAS_WIZARD_ASSET", "nas-wizard-{version}-linux-x64.tar.gz"),
        };
        std::env::var(variable).unwrap_or_else(|_| default.to_owned())
    }

    /// Staged component release path consumed by the privileged update installer.
    pub fn archive(self) -> String {
        format!("{}-update.tar", self.id())
    }

    /// The program that says the version installed on the machine.
    fn binary(self) -> Option<PathBuf> {
        let (variable, default) = match self {
            Self::Launcher => return None,
            Self::Gateway => ("NAS_GATEWAY_BINARY", "/usr/local/bin/nas-os-gateway"),
            Self::Wizard => (
                "NAS_WIZARD_BINARY",
                "/usr/local/lib/nas-wizard/wizard/bin/nas-wizard",
            ),
        };
        Some(std::env::var_os(variable).map_or_else(|| PathBuf::from(default), PathBuf::from))
    }

    /// Installed component state with an optional known version.
    pub async fn installed(self) -> Option<Option<String>> {
        let Some(binary) = self.binary() else {
            return Some(Some(env!("CARGO_PKG_VERSION").to_owned()));
        };
        if !tokio::fs::try_exists(&binary).await.unwrap_or(false) {
            return None;
        }
        let output = tokio::time::timeout(
            VERSION_TIMEOUT,
            Command::new(&binary)
                .arg("--version")
                .kill_on_drop(true)
                .output(),
        )
        .await;
        Some(match output {
            Ok(Ok(output)) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
                (!version.is_empty() && version.len() <= 64).then_some(version)
            }
            _ => None,
        })
    }
}

/// `owner/nas-gateway` beside `owner/nas-launcher`; nothing without the launcher's.
pub fn sibling(launcher: &str, name: &str) -> String {
    launcher
        .split_once('/')
        .map(|(owner, _)| format!("{owner}/{name}"))
        .unwrap_or_default()
}

/// One component as the settings show it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComponentUpdate {
    pub component: Component,
    /// The version installed, when the component can say it.
    pub current_version: Option<String>,
    /// A newer release, found by the last check.
    pub available_version: Option<String>,
    pub repository: String,
}

/// The components installed on the NAS and their updates.
#[derive(Clone, Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NasUpdates {
    pub components: Vec<ComponentUpdate>,
    pub token_configured: bool,
}

pub fn provider() -> Provider {
    match std::env::var("NAS_LAUNCHER_PROVIDER").as_deref() {
        Ok("gitlab") => Provider::Gitlab,
        _ => Provider::Github,
    }
}

/// The installed components, in the order the settings show them.
async fn installed() -> Vec<(Component, Option<String>)> {
    let mut found = Vec::new();
    for component in Component::SHOWN {
        if let Some(version) = component.installed().await {
            found.push((component, version));
        }
    }
    found
}

pub async fn state(session: &SessionData) -> NasUpdates {
    let available = &session.apps.available_updates;
    NasUpdates {
        components: installed()
            .await
            .into_iter()
            .map(|(component, current_version)| ComponentUpdate {
                component,
                current_version,
                available_version: available.get(&component).cloned(),
                repository: component.repository(),
            })
            .collect(),
        token_configured: credentials::launcher().await.is_some(),
    }
}

/// The notice of a newer release of a component.
fn available_notice(component: Component, version: &str, current: Option<&str>) -> Notice {
    let current = current.unwrap_or("-");
    if component == Component::Launcher {
        return Notice::new(
            "launcherUpdateAvailable",
            "Launcher update available",
            format!(
                "A new launcher version is available ({version}). It can be installed from Settings > Updates."
            ),
        )
        .with("version", version)
        .with("current", current);
    }
    Notice::new(
        "componentUpdateAvailable",
        "NAS update available",
        format!(
            "A new version of the {} is available ({version}). It can be installed from Settings > Updates.",
            component.id()
        ),
    )
    .with("component", component.id())
    .with("version", version)
    .with("current", current)
}

/// Checks whether a release should replace the installed component, including unknown versions.
pub fn is_update(latest: &str, current: Option<&str>) -> bool {
    current.is_none_or(|current| forge::is_newer(latest, current))
}

/// Queries available releases for installed components. The launcher repository is required.
pub async fn check(db: &SqlitePool, session: &mut SessionData) -> Result<NasUpdates, ApiFailure> {
    if Component::Launcher.repository().is_empty() {
        return Err(ApiFailure::unavailable(
            "No repository is configured for the launcher.",
        ));
    }
    if !utils::is_real(Mode::Packages) {
        return Err(ApiFailure::unavailable(
            "Package operations are not available on this installation.",
        ));
    }

    let token = credentials::launcher().await;
    let mut found = BTreeMap::new();
    for (component, current) in installed().await {
        let repository = component.repository();
        if repository.is_empty() {
            continue;
        }
        match forge::latest(provider(), &repository, token.as_deref()).await {
            Ok(release) if is_update(&release.version, current.as_deref()) => {
                found.insert(component, (release.version, current));
            }
            Ok(_) => {}
            Err(reason) if component == Component::Launcher => {
                return Err(ApiFailure::unavailable(reason));
            }
            Err(reason) => eprintln!("no update read for the {}: {reason}", component.id()),
        }
    }

    let known = std::mem::take(&mut session.apps.available_updates);
    for (component, (version, current)) in found {
        // Announces only newly detected versions across periodic update checks.
        if known.get(&component) != Some(&version) {
            let notice = available_notice(component, &version, current.as_deref());
            mail_events::forward(&notice);
            notifications::record(db, "nas-system-settings", Tone::Info, notice).await;
        }
        session.apps.available_updates.insert(component, version);
    }
    Ok(state(session).await)
}

/// Downloads every available update and hands them over to the unit that installs them together.
pub async fn install_updates(
    db: &SqlitePool,
    session: &mut SessionData,
) -> Result<String, ApiFailure> {
    let plan: Vec<(Component, String)> = session
        .apps
        .available_updates
        .iter()
        .map(|(component, version)| (*component, version.clone()))
        .collect();
    if plan.is_empty() {
        return Err(ApiFailure::bad("no update available"));
    }
    if !utils::is_real(Mode::Packages) {
        return Err(ApiFailure::unavailable(
            "Package operations are not available on this installation.",
        ));
    }

    let task = "nas-update".to_owned();
    tasks::register(
        db,
        NewTask {
            id: task.clone(),
            source: "nas-system-settings".into(),
            name: "NAS update".into(),
            detail: "downloading".into(),
            progress: None,
            pausable: false,
            cancellable: false,
        },
    )
    .await?;

    let pool = db.clone();
    let running = task.clone();
    tokio::spawn(async move {
        let outcome = fetch_and_hand_over(&pool, &running, &plan).await;
        let (state, detail) = match &outcome {
            Ok(()) => (TaskState::Done, "installing".to_owned()),
            Err(reason) => (TaskState::Failed, reason.clone()),
        };
        let _ = tasks::update(
            &pool,
            &running,
            TaskUpdate {
                detail: Some(detail),
                progress: Some(outcome.is_ok().then_some(100)),
                state: Some(state),
                ..TaskUpdate::default()
            },
        )
        .await;
    });
    Ok(task)
}

/// Downloads a component release and verifies its published checksum before installation.
pub async fn download(component: Component, version: &str) -> Result<PathBuf, String> {
    let token = credentials::launcher().await;
    let release = forge::latest(provider(), &component.repository(), token.as_deref()).await?;
    let wanted = component.asset().replace("{version}", &release.version);
    let asset = release
        .asset(&wanted)
        .ok_or_else(|| format!("the release {version} has no asset {wanted}"))?;
    let target =
        database::beside(&component.archive()).ok_or("no state directory for the update")?;
    forge::download(&asset.url, &target, token.as_deref()).await?;
    let published = release.asset(&format!("{wanted}.sha256")).ok_or_else(|| {
        format!(
            "the release of the {} published no checksum, and the NAS will not install it without one",
            component.id()
        )
    })?;
    if let Err(reason) =
        crate::services::packages::verify_against(published, &target, token.as_deref()).await
    {
        let _ = tokio::fs::remove_file(&target).await;
        return Err(reason);
    }
    Ok(target)
}

async fn fetch_and_hand_over(
    db: &SqlitePool,
    task: &str,
    plan: &[(Component, String)],
) -> Result<(), String> {
    // Removes stale archives so only components from the current update can be installed.
    for component in Component::SHOWN {
        if let Some(stale) = database::beside(&component.archive()) {
            let _ = tokio::fs::remove_file(stale).await;
        }
    }
    let share = 90 / u8::try_from(plan.len()).unwrap_or(1).max(1);
    for (index, (component, version)) in plan.iter().enumerate() {
        let _ = tasks::update(
            db,
            task,
            TaskUpdate {
                detail: Some(format!("downloading-{}", component.id())),
                progress: Some(Some(share * u8::try_from(index).unwrap_or(0))),
                ..TaskUpdate::default()
            },
        )
        .await;
        download(*component, version).await?;
    }
    utils::systemctl(&["start", "--no-block", UPDATE_UNIT], COMMAND_TIMEOUT).await
}

pub fn outcome(status: &str, version: &str) -> (Notice, Tone) {
    let (step, reason) = status.split_once(' ').unwrap_or((status, ""));
    let (notice, tone) = match step {
        // An update of the launcher alone, or one written by an older helper, which names nothing.
        "done" if reason.is_empty() || reason == "launcher" => (
            Notice::new(
                "launcherUpdated",
                "Launcher updated",
                format!("The launcher was updated to the latest version ({version})."),
            ),
            Tone::Success,
        ),
        "done" => {
            let components = reason.split_whitespace().collect::<Vec<_>>().join(", ");
            (
                Notice::new(
                    "nasUpdated",
                    "NAS updated",
                    format!(
                        "The NAS was updated ({components}). Running launcher version ({version})."
                    ),
                )
                .with("components", components),
                Tone::Success,
            )
        }
        "rolled-back" => (
            Notice::new(
                "launcherRolledBack",
                "Launcher update rolled back",
                format!("Update failed, previous version restored ({reason})."),
            ),
            Tone::Error,
        ),
        "failed" => (
            Notice::new(
                "launcherUpdateFailed",
                "Launcher update failed",
                format!("Update not started, no changes applied ({reason})."),
            ),
            Tone::Error,
        ),
        _ => (
            Notice::new(
                "launcherUpdateInterrupted",
                "Launcher update interrupted",
                format!("Update interrupted during the {step} step. Running version ({version})."),
            )
            .with("step", step),
            Tone::Warning,
        ),
    };
    (notice.with("version", version).with("reason", reason), tone)
}

/// Polling interval and retry count for updates still active during launcher startup.
const SETTLE_EVERY: Duration = Duration::from_secs(3);
const SETTLE_TRIES: u32 = 200;

/// Terminal update steps distinguished from active or interrupted work.
fn finished(status: &str) -> bool {
    let step = status.split_once(' ').map_or(status, |(step, _)| step);
    matches!(step, "done" | "rolled-back" | "failed")
}

async fn update_running() -> bool {
    utils::systemctl(
        &["is-active", "--quiet", UPDATE_UNIT],
        Duration::from_secs(5),
    )
    .await
    .is_ok()
}

/// Reports the last completed update after its unit exits, preserving active update state.
pub async fn report_previous(db: &SqlitePool) {
    let Some(path) = database::beside("launcher-update.status") else {
        return;
    };
    let db = db.clone();
    tokio::spawn(async move {
        settle(&db, &path, SETTLE_EVERY, SETTLE_TRIES, update_running).await;
    });
}

/// Waits, `every` at a time and `tries` times at most, while `running` says the update is still
/// going, then records how it ended and removes the status.
pub async fn settle<F, Running>(
    db: &SqlitePool,
    path: &std::path::Path,
    every: Duration,
    tries: u32,
    running: F,
) where
    F: Fn() -> Running,
    Running: std::future::Future<Output = bool>,
{
    for attempt in 0..=tries {
        let Ok(status) = tokio::fs::read_to_string(path).await else {
            return;
        };
        let status = status.trim().to_owned();
        if !finished(&status) && attempt < tries && running().await {
            tokio::time::sleep(every).await;
            continue;
        }
        let _ = tokio::fs::remove_file(path).await;
        let (mut notice, tone) = outcome(&status, env!("CARGO_PKG_VERSION"));
        if matches!(tone, Tone::Error | Tone::Warning) {
            notice = notice.detail("update", &status, Some(UPDATE_UNIT)).await;
        }
        mail_events::forward(&notice);
        notifications::record(db, "nas-system-settings", tone, notice).await;
        return;
    }
}

#[cfg(test)]
#[path = "../../../../tests/apps/services/nas_system_settings/updates.rs"]
mod tests;
