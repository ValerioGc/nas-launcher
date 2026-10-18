//! Installs packages in background tasks when `NAS_PACKAGES_MODE=real`.
//! Downloads are staged and verified when a checksum is published before replacing the installed copy.

pub mod catalogue;
pub mod credentials;
pub mod forge;
pub mod logos;

use crate::{
    http::error::ApiFailure,
    services::{
        database, desktop, grants, manifest,
        notifications::{self, Notice, Tone},
        packages::catalogue::CatalogueApp,
        registry, shares, sync_ports, system_packages,
    },
    utils::{self, Mode},
    widgets::services::tasks::{self, NewTask, TaskState, TaskUpdate},
};
use serde::Serialize;
use sqlx::SqlitePool;
use std::{collections::HashMap, path::PathBuf, time::Duration};
use tokio::{io::AsyncWriteExt, process::Command};

const UNPACK_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Copy)]
enum InstallContext {
    Interactive,
    Setup,
}

impl InstallContext {
    fn notify(self) -> bool {
        matches!(self, Self::Interactive)
    }

    async fn add_shortcut(self, db: &SqlitePool, id: &str) -> Result<(), String> {
        if matches!(self, Self::Setup) || desktop::shortcut_on_install(db).await {
            desktop::add_app(db, id)
                .await
                .map_err(|failure| format!("cannot create the desktop shortcut: {}", failure.1))?;
        }
        Ok(())
    }
}

/// Package directory shared with [`crate::services::manifest`].
pub fn apps_directory() -> PathBuf {
    if let Ok(configured) = std::env::var("NAS_APPS_DIR") {
        return PathBuf::from(configured);
    }
    database::beside("apps").unwrap_or_else(|| PathBuf::from("/var/lib/nas-os/apps"))
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub id: String,
    pub name: String,
    pub system: bool,
    pub info: String,
    pub repository: String,
    pub logo: String,
    pub installed_version: Option<String>,
    pub latest_version: Option<String>,
    pub update_available: bool,
}

fn disabled() -> ApiFailure {
    ApiFailure::unavailable("Package operations are not available on this installation.")
}

/// Crosses the catalogue with installed versions without contacting the forge.
pub async fn offers(db: &SqlitePool) -> Result<Vec<Offer>, ApiFailure> {
    let installed = installed_versions(db).await?;
    let logos = logos::addresses(db).await;
    Ok(catalogue::read(db)
        .await
        .into_iter()
        .map(|app| {
            let installed_version = installed.get(&app.id).cloned();
            Offer {
                logo: logos.get(&app.id).cloned().unwrap_or_default(),
                id: app.id,
                name: app.name,
                system: app.system,
                info: app.info,
                repository: app.repository,
                installed_version,
                latest_version: None,
                update_available: false,
            }
        })
        .collect())
}

/// Queries the latest release for each catalogue application.
pub async fn check(db: &SqlitePool) -> Result<Vec<Offer>, ApiFailure> {
    if !utils::is_real(Mode::Packages) {
        return Err(disabled());
    }

    let catalogue = catalogue::read_config(db).await;
    let token = credentials::launcher().await;
    let versions = installed_versions(db).await?;
    let logos = logos::addresses(db).await;
    let mut found = Vec::new();
    for app in catalogue.apps {
        let installed = versions.get(&app.id).cloned();
        let latest = forge::latest(app.provider, &app.repository, token.as_deref())
            .await
            .map_err(|reason| {
                eprintln!("cannot check {}: {reason}", app.repository);
                reason
            })
            .ok();

        let update_available = match (&latest, &installed) {
            (Some(release), Some(version)) => forge::is_newer(&release.version, version),
            _ => false,
        };
        found.push(Offer {
            logo: logos.get(&app.id).cloned().unwrap_or_default(),
            id: app.id,
            name: app.name,
            system: app.system,
            info: app.info,
            repository: app.repository,
            installed_version: installed,
            latest_version: latest.map(|release| release.version),
            update_available,
        });
    }

    Ok(found)
}

/// The notice of the offers that have a newer release, when some have.
pub fn updates_notice(found: &[Offer]) -> Option<Notice> {
    let updates = found.iter().filter(|offer| offer.update_available).count();
    if updates == 0 {
        return None;
    }
    let names = found
        .iter()
        .filter(|offer| offer.update_available)
        .map(|offer| offer.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Some(
        Notice::new(
            "appUpdatesAvailable",
            "Updates available",
            "Updates are available for some applications in the Application manager.",
        )
        .with("count", updates.to_string())
        .with("apps", names),
    )
}

/// Creates a notification when some of the offers have a newer release.
pub async fn announce(db: &SqlitePool, found: &[Offer]) {
    if let Some(notice) = updates_notice(found) {
        notifications::record(db, "nas-app-manager", Tone::Info, notice).await;
    }
}

/// Returns a task ID immediately, download and installation run in the background.
pub async fn start_install(db: &SqlitePool, id: &str) -> Result<String, ApiFailure> {
    let (app, task) = begin_install(db, id).await?;
    let pool = db.clone();
    let running = task.clone();
    tokio::spawn(async move {
        let log = install_log(&running);
        begin_log(&log, &app).await;
        let context = InstallContext::Interactive;
        let outcome = run_install(&pool, &app, &running, &log, context).await;
        if let Err(reason) = &outcome {
            trace(&log, &format!("FAILED\n{reason}")).await;
        }
        finish(&pool, &running, &app, outcome, context).await;
    });
    Ok(task)
}

/// Installs an initial application, retaining task state and logs without creating notifications.
pub async fn install_for_setup(db: &SqlitePool, id: &str) -> Result<(), String> {
    let (app, task) = begin_install(db, id)
        .await
        .map_err(|ApiFailure(_, reason)| reason)?;
    let log = install_log(&task);
    begin_log(&log, &app).await;
    let context = InstallContext::Setup;
    let outcome = run_install(db, &app, &task, &log, context).await;
    if let Err(reason) = &outcome {
        trace(&log, &format!("FAILED\n{reason}")).await;
    }
    let result = outcome.as_ref().map(|_| ()).map_err(Clone::clone);
    finish(db, &task, &app, outcome, context).await;
    result
}

async fn begin_install(db: &SqlitePool, id: &str) -> Result<(CatalogueApp, String), ApiFailure> {
    if !utils::is_real(Mode::Packages) {
        return Err(disabled());
    }
    let app = catalogue::find(db, id)
        .await
        .ok_or_else(|| ApiFailure::missing(format!("unknown application: {id}")))?;

    let task = format!("install-{id}");
    tasks::register(
        db,
        NewTask {
            id: task.clone(),
            source: "nas-app-manager".into(),
            name: app.name.clone(),
            detail: String::new(),
            progress: Some(0),
            pausable: false,
            cancellable: false,
        },
    )
    .await?;
    Ok((app, task))
}

fn install_log(task: &str) -> PathBuf {
    std::env::var_os("NAS_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/log/nas-os"))
        .join(format!("{task}.log"))
}

async fn begin_log(path: &std::path::Path, app: &CatalogueApp) {
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let _ = tokio::fs::write(
        path,
        format!(
            "Application installation\nID: {}\nRepository: {}\nProvider: {:?}\nAsset template: {}\n\n",
            app.id, app.repository, app.provider, app.asset
        ),
    )
    .await;
}

async fn trace(path: &std::path::Path, text: &str) {
    let Ok(mut file) = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
    else {
        return;
    };
    let _ = file.write_all(text.as_bytes()).await;
    let _ = file.write_all(b"\n\n").await;
}

/// The asset of `release` the catalogue names for `app`, written down in the log of the task.
async fn select_asset(
    app: &CatalogueApp,
    release: &forge::Release,
    log: &std::path::Path,
) -> Result<forge::Asset, String> {
    trace(
        log,
        &format!(
            "Release selected: {}\nPublished assets: {}",
            release.version,
            release
                .assets
                .iter()
                .map(|asset| asset.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )
    .await;
    let wanted = app.asset.replace("{version}", &release.version);
    let asset = release
        .asset_like(&wanted)
        .ok_or_else(|| missing_asset(app, release, &wanted))?
        .clone();
    trace(
        log,
        &format!("Asset selected: {}\nDownloading package", asset.name),
    )
    .await;
    Ok(asset)
}

/// Checks the downloaded `archive` against the checksum the release published beside it. Without
/// one the package goes in unverified, and an interactive install says so in a notification.
async fn verify_published(
    db: &SqlitePool,
    app: &CatalogueApp,
    release: &forge::Release,
    asset: &forge::Asset,
    archive: &std::path::Path,
    log: &std::path::Path,
    context: InstallContext,
) -> Result<(), String> {
    let token = credentials::launcher().await;
    // The checksum is named after the file the release really published.
    let wanted = &asset.name;
    if let Some(published) = release.asset(&format!("{wanted}.sha256")) {
        return verify_against_logged(published, archive, token.as_deref(), Some(log)).await;
    }
    if context.notify() {
        notifications::record(
            db,
            "nas-app-manager",
            Tone::Warning,
            Notice::new(
                "packageNotVerified",
                "Package not verified",
                format!(
                    "No checksum of {wanted} published by {}. Package of {} installed without verification.",
                    app.repository, app.name
                ),
            )
            .with("app", app.name.clone())
            .with("repository", app.repository.clone())
            .with("asset", wanted.clone())
            .log(log.display().to_string()),
        )
        .await;
    }
    Ok(())
}

/// Validates and applies package requirements before replacing an installation.
/// Debian dependencies are installed before path grants. Unsupported requests
/// are rejected without installing the application.
async fn prepare_unpacked(
    db: &SqlitePool,
    app: &CatalogueApp,
    task: &str,
    staging: &std::path::Path,
    log: &std::path::Path,
) -> Result<(), String> {
    let unpacked = manifest::read_unpacked(staging, &app.id)
        .await
        .ok_or("the package carries no manifest this launcher reads")?;
    let granted = grants::checked(&unpacked.grants_asked())?;
    let needed = system_packages::checked(&unpacked.packages)?;
    if !needed.is_empty() {
        say(db, task, "system packages", Some(90)).await;
        trace(
            log,
            &format!("Installing system packages: {}", needed.join(", ")),
        )
        .await;
        system_packages::install(&app.id, &needed).await?;
    }
    let listed = if granted.is_empty() {
        "none".to_owned()
    } else {
        granted.join(", ")
    };
    trace(log, &format!("Granted paths: {listed}")).await;
    grants::apply(&app.id, &granted).await?;
    // Disables the default Samba configuration until a NAS folder is shared.
    if shares::may_share(&unpacked) {
        shares::refresh().await;
    }
    if sync_ports::may_sync(&unpacked) {
        sync_ports::refresh().await;
    }
    Ok(())
}

/// Puts the unpacked `staging` in the place of the installed version, which is set aside first and
/// put back if the new one cannot take its place.
async fn swap_into_place(
    root: &std::path::Path,
    id: &str,
    staging: &std::path::Path,
) -> Result<(), String> {
    let target = root.join(id);
    let previous = root.join(format!(".{id}.old"));
    let _ = tokio::fs::remove_dir_all(&previous).await;
    if tokio::fs::metadata(&target).await.is_ok() {
        tokio::fs::rename(&target, &previous)
            .await
            .map_err(|error| format!("cannot set the previous version aside: {error}"))?;
    }
    if let Err(error) = tokio::fs::rename(staging, &target).await {
        // Put back what was working before giving up.
        let _ = tokio::fs::rename(&previous, &target).await;
        return Err(format!("cannot put the new version in place: {error}"));
    }
    let _ = tokio::fs::remove_dir_all(&previous).await;
    Ok(())
}

async fn run_install(
    db: &SqlitePool,
    app: &CatalogueApp,
    task: &str,
    log: &std::path::Path,
    context: InstallContext,
) -> Result<String, String> {
    let new_install = !crate::apps::services::nas_app_manager::is_installed(db, &app.id)
        .await
        .map_err(|failure| failure.1)?;
    let token = credentials::launcher().await;
    let release =
        forge::latest_logged(app.provider, &app.repository, token.as_deref(), Some(log)).await?;
    let asset = select_asset(app, &release, log).await?;

    let root = apps_directory();
    let staging = root.join(format!(".{}.new", app.id));
    let archive = root.join(format!(".{}.tar", app.id));
    tokio::fs::create_dir_all(&root)
        .await
        .map_err(|error| format!("cannot create {}: {error}", root.display()))?;
    let _ = tokio::fs::remove_dir_all(&staging).await;

    say(
        db,
        task,
        &format!("{} {}", release.version, asset.name),
        Some(0),
    )
    .await;
    forge::download_logged(&asset.url, &archive, token.as_deref(), Some(log)).await?;
    trace(log, "Package downloaded\nVerifying checksum").await;
    say(db, task, "verifying", Some(70)).await;
    verify_published(db, app, &release, &asset, &archive, log, context).await?;

    say(db, task, "unpacking", Some(85)).await;
    trace(log, "Checksum accepted\nUnpacking package").await;
    unpack(&archive, &staging).await?;
    let _ = tokio::fs::remove_file(&archive).await;

    if let Err(reason) = prepare_unpacked(db, app, task, &staging, log).await {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(reason);
    }
    swap_into_place(&root, &app.id, &staging).await?;

    record_installed(db, app, &release.version).await?;
    if new_install || matches!(context, InstallContext::Setup) {
        context.add_shortcut(db, &app.id).await?;
    }
    trace(log, &format!("Installed version {}", release.version)).await;
    if crate::services::lifecycle::is_running(&app.id).await {
        crate::services::lifecycle::restart_after_install(db, &app.id, context.notify()).await;
    }
    Ok(release.version)
}

fn missing_asset(app: &CatalogueApp, release: &forge::Release, wanted: &str) -> String {
    let published = release
        .assets
        .iter()
        .map(|asset| asset.name.as_str())
        .collect::<Vec<_>>();
    format!(
        "the latest release of {}, {}, has no asset {wanted}; it publishes {}",
        app.repository,
        release.version,
        if published.is_empty() {
            "none".to_owned()
        } else {
            published.join(", ")
        }
    )
}

/// Compares the archive with the checksum published beside it.
pub async fn verify_against(
    published: &forge::Asset,
    archive: &std::path::Path,
    token: Option<&str>,
) -> Result<(), String> {
    verify_against_logged(published, archive, token, None).await
}

async fn verify_against_logged(
    published: &forge::Asset,
    archive: &std::path::Path,
    token: Option<&str>,
    log: Option<&std::path::Path>,
) -> Result<(), String> {
    let sums = archive.with_extension("sha256");
    forge::download_logged(&published.url, &sums, token, log).await?;

    let declared = tokio::fs::read_to_string(&sums)
        .await
        .map_err(|error| format!("cannot read the checksum: {error}"))?
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_lowercase();

    let _ = tokio::fs::remove_file(&sums).await;
    let actual = digest(archive).await?;
    if declared != actual {
        let _ = tokio::fs::remove_file(archive).await;
        return Err("the package does not match its published checksum".to_owned());
    }
    Ok(())
}

/// SHA-256 of a file
async fn digest(path: &std::path::Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncReadExt;

    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|error| format!("cannot read the package: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|error| format!("cannot read the package: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

async fn unpack(archive: &std::path::Path, into: &std::path::Path) -> Result<(), String> {
    tokio::fs::create_dir_all(into)
        .await
        .map_err(|error| format!("cannot create {}: {error}", into.display()))?;

    let extract = Command::new("tar")
        .arg("--extract")
        .arg("--auto-compress")
        .arg("--no-same-owner")
        .arg("--file")
        .arg(archive)
        .arg("--directory")
        .arg(into)
        .output();

    match tokio::time::timeout(UNPACK_TIMEOUT, extract).await {
        Ok(Ok(output)) if output.status.success() => share(into).await,
        Ok(Ok(output)) => Err(String::from_utf8_lossy(&output.stderr)
            .lines()
            .next()
            .unwrap_or("tar refused the package")
            .trim()
            .to_owned()),
        Ok(Err(error)) => Err(format!("cannot run tar: {error}")),
        Err(_) => Err("unpacking did not finish".to_owned()),
    }
}

async fn share(package: &std::path::Path) -> Result<(), String> {
    let status = Command::new("chmod")
        .arg("-R")
        .arg("g=u-w")
        .arg(package)
        .status()
        .await
        .map_err(|error| format!("cannot run chmod: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("cannot open the package to its process".to_owned())
    }
}

async fn say(db: &SqlitePool, task: &str, detail: &str, progress: Option<u8>) {
    let _ = tasks::update(
        db,
        task,
        TaskUpdate {
            detail: Some(detail.to_owned()),
            progress: Some(progress),
            ..TaskUpdate::default()
        },
    )
    .await;
}

async fn finish(
    db: &SqlitePool,
    task: &str,
    app: &CatalogueApp,
    outcome: Result<String, String>,
    context: InstallContext,
) {
    let (state, detail) = match &outcome {
        Ok(version) => (TaskState::Done, format!("version {version}")),
        Err(reason) => (TaskState::Failed, reason.clone()),
    };

    let update = TaskUpdate {
        detail: Some(detail.clone()),
        progress: Some(outcome.is_ok().then_some(100)),
        state: Some(state),
        ..TaskUpdate::default()
    };
    let _ = if context.notify() {
        tasks::update(db, task, update).await
    } else {
        tasks::update_without_notification(db, task, update).await
    };

    if let (Ok(version), true) = (outcome, context.notify()) {
        notifications::record(
            db,
            "nas-app-manager",
            Tone::Success,
            Notice::new(
                "appInstalled",
                "Application installed",
                format!("New version of {} installed ({version}).", app.name),
            )
            .with("app", app.name.clone())
            .with("version", version),
        )
        .await;
    }
}

async fn record_installed(
    db: &SqlitePool,
    app: &CatalogueApp,
    version: &str,
) -> Result<(), String> {
    let row = registry::Installed {
        id: app.id.clone(),
        name: app.name.clone(),
        info: app.info.clone(),
        version: version.to_owned(),
    };
    registry::record(db, &row)
        .await
        .map_err(|error| format!("cannot write the register: {error}"))
}

pub async fn remove_package(id: &str) {
    let root = apps_directory();
    for name in [id.to_owned(), format!(".{id}.old"), format!(".{id}.new")] {
        let _ = tokio::fs::remove_dir_all(root.join(name)).await;
    }
}

async fn installed_versions(db: &SqlitePool) -> Result<HashMap<String, String>, ApiFailure> {
    let rows = registry::installed(db)
        .await
        .map_err(|_| ApiFailure::unavailable("application manager unavailable"))?
        .into_iter()
        .map(|row| (row.id, row.version))
        .collect();
    Ok(merge_versions(rows, crate::services::manifest::all().await))
}

fn merge_versions(
    rows: Vec<(String, String)>,
    manifests: Vec<crate::services::manifest::Manifest>,
) -> HashMap<String, String> {
    let mut versions: HashMap<_, _> = rows.into_iter().collect();
    for found in manifests {
        if !found.version.is_empty() || !versions.contains_key(&found.id) {
            versions.insert(found.id, found.version);
        }
    }
    versions
}

#[cfg(test)]
#[path = "../../tests/services/packages.rs"]
mod tests;
