use crate::{
    http::error::ApiFailure,
    services::{
        background, desktop, grants, lifecycle, mail_events, manifest,
        notifications::{self, Notice, Tone},
        packages::{self, catalogue, logos, Offer},
        registry,
        settings::{self, Section},
        shares, sync_ports, system_packages,
    },
    utils::{self, Mode},
};
use catalogue::{CatalogueApp, Category};
use manifest::Manifest;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
    time::{Duration, Instant},
};

const WATCH_TICK: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Frequency {
    Hourly,
    Daily,
    Weekly,
}

impl Frequency {
    pub fn period(self) -> Duration {
        Duration::from_secs(match self {
            Self::Hourly => 60 * 60,
            Self::Daily => 24 * 60 * 60,
            Self::Weekly => 7 * 24 * 60 * 60,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ManagerSettings {
    pub auto_check: bool,
    pub frequency: Frequency,
    /// Looks for newer releases each time the App Manager is opened.
    #[serde(default)]
    pub check_on_open: bool,
    /// Installs what that check finds: it means nothing without the check.
    #[serde(default)]
    pub update_on_open: bool,
}

impl Default for ManagerSettings {
    fn default() -> Self {
        Self {
            auto_check: false,
            frequency: Frequency::Daily,
            check_on_open: false,
            update_on_open: false,
        }
    }
}

impl ManagerSettings {
    pub fn validate(&self) -> Result<(), ApiFailure> {
        if self.update_on_open && !self.check_on_open {
            return Err(ApiFailure::bad("updateOnOpen requires checkOnOpen"));
        }
        Ok(())
    }
}

pub async fn load_settings(db: &SqlitePool) -> Result<ManagerSettings, ApiFailure> {
    settings::read(db, Section::AppManager).await
}

pub async fn save_settings(
    db: &SqlitePool,
    input: ManagerSettings,
) -> Result<ManagerSettings, ApiFailure> {
    input.validate()?;
    settings::write(db, Section::AppManager, &input).await?;
    settings::record_saved(db, Section::AppManager).await;
    Ok(input)
}

pub fn due(last: Option<Instant>, now: Instant, frequency: Frequency) -> bool {
    last.is_none_or(|at| now.duration_since(at) >= frequency.period())
}

pub fn newer_releases(found: &[Offer]) -> BTreeSet<String> {
    found
        .iter()
        .filter(|offer| offer.update_available)
        .filter_map(|offer| {
            offer
                .latest_version
                .as_ref()
                .map(|version| format!("{}@{version}", offer.id))
        })
        .collect()
}

pub async fn watch_updates(db: SqlitePool) {
    let mut last = None;
    let mut announced = BTreeSet::new();
    loop {
        let wanted = load_settings(&db).await.unwrap_or_default();
        if wanted.auto_check
            && utils::is_real(Mode::Packages)
            && due(last, Instant::now(), wanted.frequency)
        {
            last = Some(Instant::now());
            match packages::check(&db).await {
                Ok(found) => {
                    let fresh = newer_releases(&found);
                    if !fresh.is_subset(&announced) {
                        packages::announce(&db, &found).await;
                        // Only the automatic check mails: a check asked for is answered on screen.
                        if let Some(notice) = packages::updates_notice(&found) {
                            mail_events::forward(&notice);
                        }
                    }
                    announced = fresh;
                }
                Err(failure) => eprintln!("automatic update check failed: {}", failure.1),
            }
        }
        tokio::time::sleep(WATCH_TICK).await;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct App {
    pub id: String,
    pub name: String,
    pub info: String,
    pub system: bool,
    pub version: String,
    pub running: bool,
    pub icon: String,
    pub category: Option<Category>,
}

impl App {
    fn system(app: &registry::SystemApp) -> Self {
        Self {
            id: app.id.to_owned(),
            name: app.name.to_owned(),
            info: app.info.to_owned(),
            system: true,
            version: app.version(),
            running: true,
            icon: String::new(),
            category: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct AppDetail {
    pub name: String,
    pub version: String,
    pub description: String,
    pub logo: String,
    pub url_repo: String,
    pub website: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct ActionAck {
    pub id: String,
    pub action: &'static str,
    pub accepted: bool,
    pub task: Option<String>,
}

/// Installed apps according to the database, excluding the built-in shell apps.
async fn rows(db: &SqlitePool) -> Result<Vec<App>, ApiFailure> {
    let rows = registry::installed(db).await.map_err(|_| unavailable())?;
    Ok(rows
        .into_iter()
        .map(|row| App {
            id: row.id,
            name: row.name,
            info: row.info,
            system: false,
            version: row.version,
            running: false,
            icon: String::new(),
            category: None,
        })
        .collect())
}

fn merge(mut apps: Vec<App>, manifests: Vec<Manifest>, catalogue: &[CatalogueApp]) -> Vec<App> {
    for found in manifests {
        let icon = icon_url(&found);
        if let Some(app) = apps.iter_mut().find(|app| app.id == found.id) {
            if !found.version.is_empty() {
                app.version = found.version;
            }
            app.icon = icon;
            app.category = found.category;
            app.system = found.system;
            continue;
        }

        let described = catalogue.iter().find(|entry| entry.id == found.id);
        apps.push(App {
            id: found.id,
            name: described.map_or(found.name, |entry| entry.name.clone()),
            info: described
                .map(|entry| entry.info.clone())
                .unwrap_or_default(),
            system: found.system,
            version: found.version,
            running: false,
            icon,
            category: found.category,
        });
    }
    apps
}

fn icon_url(found: &Manifest) -> String {
    if found.icon.is_empty() {
        String::new()
    } else {
        format!("/api/apps/{}/icon", found.id)
    }
}

async fn external(db: &SqlitePool) -> Result<Vec<App>, ApiFailure> {
    let catalogue = catalogue::read(db).await;
    let logos = logos::addresses(db).await;
    let mut apps = merge(rows(db).await?, manifest::all().await, &catalogue);
    complete(&mut apps, &catalogue, &logos);
    Ok(apps)
}

fn complete(apps: &mut [App], catalogue: &[CatalogueApp], logos: &HashMap<String, String>) {
    for app in apps {
        if app.icon.is_empty() {
            app.icon = logos.get(&app.id).cloned().unwrap_or_default();
        }
        if let Some(entry) = catalogue.iter().find(|entry| entry.id == app.id) {
            app.category = entry.category;
            app.system |= entry.system;
        }
    }
}

pub async fn sync_preinstalled(db: &SqlitePool) -> Result<(), sqlx::Error> {
    sync_manifests(db, manifest::all().await, &catalogue::read(db).await).await
}

async fn sync_manifests(
    db: &SqlitePool,
    manifests: Vec<Manifest>,
    catalogue: &[CatalogueApp],
) -> Result<(), sqlx::Error> {
    let registered = registry::installed(db).await?;
    for found in manifests {
        if registry::is_system(&found.id) {
            continue;
        }
        let described = catalogue.iter().find(|app| app.id == found.id);
        let existing = registered.iter().find(|app| app.id == found.id);
        let row = registry::Installed {
            id: found.id,
            name: described.map_or_else(|| found.name.clone(), |app| app.name.clone()),
            info: described.map_or_else(
                || existing.map_or(String::new(), |app| app.info.clone()),
                |app| app.info.clone(),
            ),
            version: if found.version.is_empty() {
                existing.map_or(String::new(), |app| app.version.clone())
            } else {
                found.version
            },
        };
        if existing != Some(&row) {
            registry::record(db, &row).await?;
        }
    }
    Ok(())
}

pub async fn installed(db: &SqlitePool) -> Result<Vec<App>, ApiFailure> {
    let mut apps = registry::SYSTEM.iter().map(App::system).collect::<Vec<_>>();
    let mut external = external(db).await?;

    let running = lifecycle::running(
        &external
            .iter()
            .map(|app| app.id.clone())
            .collect::<Vec<_>>(),
    )
    .await;
    for app in &mut external {
        app.running = running.get(&app.id).copied().unwrap_or(false);
    }

    apps.extend(external);
    Ok(apps)
}

pub async fn available(db: &SqlitePool) -> Result<Vec<packages::Offer>, ApiFailure> {
    Ok(packages::offers(db)
        .await?
        .into_iter()
        .filter(|offer| offer.installed_version.is_none())
        .collect())
}

pub async fn is_installed(db: &SqlitePool, id: &str) -> Result<bool, ApiFailure> {
    if registry::is_system(id) || manifest::read(id).await.is_some() {
        return Ok(true);
    }

    Ok(rows(db).await?.iter().any(|app| app.id == id))
}

pub async fn detail(db: &SqlitePool, id: &str) -> Result<AppDetail, ApiFailure> {
    if let Some(app) = registry::system(id) {
        return Ok(AppDetail {
            name: app.name.to_owned(),
            version: app.version(),
            description: app.info.to_owned(),
            logo: String::new(),
            url_repo: String::new(),
            website: String::new(),
        });
    }

    let installed = external(db).await?.into_iter().find(|app| app.id == id);
    let offered = catalogue::find(db, id).await;
    let (name, description) = match (&installed, &offered) {
        (Some(app), _) => (app.name.clone(), app.info.clone()),
        (None, Some(app)) => (app.name.clone(), app.info.clone()),
        (None, None) => return Err(ApiFailure::missing(format!("unknown application: {id}"))),
    };

    let url_repo = offered
        .as_ref()
        .map(|app| match app.provider {
            catalogue::Provider::Github => format!("https://github.com/{}", app.repository),
            catalogue::Provider::Gitlab => format!("https://gitlab.com/{}", app.repository),
        })
        .unwrap_or_default();

    let logo = match &installed {
        Some(app) if !app.icon.is_empty() => app.icon.clone(),
        _ => logos::addresses(db).await.remove(id).unwrap_or_default(),
    };
    Ok(AppDetail {
        name,
        version: installed.map(|app| app.version).unwrap_or_default(),
        description,
        logo,
        url_repo,
        website: String::new(),
    })
}

pub async fn install(db: &SqlitePool, id: String) -> Result<ActionAck, ApiFailure> {
    if registry::is_system(&id) {
        return Err(ApiFailure::bad(
            "system apps cannot be installed or removed",
        ));
    }

    let task = packages::start_install(db, &id).await?;
    Ok(ActionAck {
        id,
        action: "install",
        accepted: true,
        task: Some(task),
    })
}

pub async fn control(db: &SqlitePool, id: String, action: &str) -> Result<ActionAck, ApiFailure> {
    // Its window closes: stopped, unless it has work to do without it.
    let closing = action == "close";
    let action = if closing { "stop" } else { action };
    let Some(wanted) = lifecycle::Action::parse(action) else {
        return Err(ApiFailure::bad("invalid application action"));
    };
    if registry::is_system(&id) {
        return Err(ApiFailure::bad(
            "system apps are part of the launcher: they are not started or stopped",
        ));
    }
    if !is_installed(db, &id).await? {
        return Err(ApiFailure::missing(format!("unknown application: {id}")));
    }

    if closing && background::keeps_running(db, &id).await {
        return Ok(ActionAck {
            id,
            action: "close",
            accepted: true,
            task: None,
        });
    }
    lifecycle::run(&id, wanted).await?;
    Ok(ActionAck {
        id,
        action: if closing { "close" } else { wanted.verb() },
        accepted: true,
        task: None,
    })
}

/// What a session finds when it begins or leaves when it ends: the applications its windows
/// started are stopped, those with work to do without a window keep running. The ids stopped.
pub async fn close_all(db: &SqlitePool) -> Result<Vec<String>, ApiFailure> {
    let ids: Vec<String> = registry::installed(db)
        .await
        .map_err(|_| unavailable())?
        .into_iter()
        .map(|app| app.id)
        .filter(|id| !registry::is_system(id))
        .collect();
    let states = lifecycle::running(&ids).await;
    let mut stopped = Vec::new();
    for id in ids {
        if !states.get(&id).copied().unwrap_or(false) || background::keeps_running(db, &id).await {
            continue;
        }
        match lifecycle::run(&id, lifecycle::Action::Stop).await {
            Ok(()) => stopped.push(id),
            Err(failure) => eprintln!("{id} was left running: {}", failure.1),
        }
    }
    Ok(stopped)
}

pub async fn uninstall(
    db: &SqlitePool,
    id: String,
    remove_data: bool,
) -> Result<ActionAck, ApiFailure> {
    if registry::is_system(&id) {
        return Err(ApiFailure::bad(
            "system apps cannot be installed or removed",
        ));
    }
    if !is_installed(db, &id).await? {
        return Err(ApiFailure::missing(format!("unknown application: {id}")));
    }

    let name = rows(db)
        .await?
        .into_iter()
        .find(|app| app.id == id)
        .map_or_else(|| id.clone(), |app| app.name);
    registry::forget(db, &id).await.map_err(|_| unavailable())?;
    desktop::forget_app(db, &id).await;
    lifecycle::stop_before_removal(&id).await;
    if remove_data {
        lifecycle::remove_data(&id).await;
    }
    packages::remove_package(&id).await;
    if let Err(reason) = grants::revoke(&id).await {
        eprintln!("{reason}");
    }
    shares::revoke(&id).await;
    sync_ports::revoke(&id).await;
    // apt takes its time: the removal answers at once and the packages go after it.
    let removed = id.clone();
    tokio::spawn(async move {
        if let Err(reason) = system_packages::remove(&removed).await {
            eprintln!("{reason}");
        }
    });

    notifications::record(
        db,
        "nas-app-manager",
        Tone::Success,
        Notice::new(
            "appRemoved",
            "Application removed",
            format!("{name} removed from the NAS along with its package."),
        )
        .with("app", name),
    )
    .await;
    Ok(ActionAck {
        id,
        action: "uninstall",
        accepted: true,
        task: None,
    })
}

pub async fn icon(id: &str) -> Result<(&'static str, Vec<u8>), ApiFailure> {
    const LIMIT: u64 = 512 * 1024;

    let found = manifest::read(id)
        .await
        .ok_or_else(|| ApiFailure::missing(format!("unknown application: {id}")))?;
    if found.icon.is_empty() {
        return Err(ApiFailure::missing(format!("{id} declares no icon")));
    }

    let relative = std::path::Path::new(&found.icon);
    let mime = match relative
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        other => return Err(ApiFailure::bad(format!("{other} is not an icon format"))),
    };

    let missing = || ApiFailure::missing(format!("{id} has no {}", found.icon));
    let file = inside(&packages::apps_directory().join(id), relative)
        .await
        .ok_or_else(missing)??;
    let size = tokio::fs::metadata(&file)
        .await
        .map_err(|_| missing())?
        .len();
    if size > LIMIT {
        return Err(ApiFailure::bad(
            "the icon is larger than the launcher will serve",
        ));
    }

    let bytes = tokio::fs::read(&file)
        .await
        .map_err(|error| ApiFailure::unavailable(format!("cannot read the icon: {error}")))?;
    Ok((mime, bytes))
}

async fn inside(
    package: &std::path::Path,
    relative: &std::path::Path,
) -> Option<Result<PathBuf, ApiFailure>> {
    let root = tokio::fs::canonicalize(package).await.ok()?;
    let file = tokio::fs::canonicalize(package.join(relative)).await.ok()?;
    Some(if file.starts_with(&root) {
        Ok(file)
    } else {
        Err(ApiFailure::bad("the icon has to stay inside the package"))
    })
}

fn unavailable() -> ApiFailure {
    ApiFailure::unavailable("application manager unavailable")
}

#[cfg(test)]
#[path = "../../../../tests/apps/services/nas_app_manager.rs"]
mod tests;
