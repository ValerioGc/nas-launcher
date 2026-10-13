//! The configuration of the NAS carried out to a file and back: the general settings without the
//! name of the NAS, the access settings and the allowed addresses, the power schedule, the refresh
//! of the widgets and the desktop, and the settings of the applications chosen at the export. Never
//! the network, the name, the account or the token.
//!
//! The launcher sees the configuration in clear: the page encrypts the file with a password of its
//! own before saving it, and decrypts it before importing it. Both ways need the password of the
//! account again, checked by the gateway (`http::extract::Reauthenticated`).

use super::{
    access, power::schedule, Access, Background, DesktopIconSize, General, Language, Power, Theme,
    Themes,
};
use crate::{
    apps::services::{
        nas_app_manager::{self, ManagerSettings},
        nas_notification_manager::NotificationSettings,
        nas_system_terminal::TerminalSettings,
    },
    http::error::ApiFailure,
    services::{
        desktop::{self, ShortcutKind},
        notifications::{self, Notice, Tone},
        packages::catalogue::{self, Catalogue, CatalogueApp},
        registry,
        settings::{self, Section},
    },
    utils::{self, now_ms, Mode},
    widgets::services::settings::{self as widgets, WidgetSettings},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashSet;

pub const FORMAT: &str = "nas-os-config";
/// The shape of the file: a newer one is refused, an older one is read as it comes. Version 2 added
/// the desktop and the applications.
pub const VERSION: u32 = 2;
const MAX_RULES: usize = 256;
const MAX_SHORTCUTS: usize = 500;
const MAX_INSTALLED: usize = 128;

const APP_MANAGER: &str = "nas-app-manager";
const NOTIFICATIONS: &str = "nas-notification-manager";
const TERMINAL: &str = "nas-system-terminal";
/// The applications whose settings can go in the file. The others keep none on the NAS yet.
pub const EXPORTABLE: [&str; 3] = [APP_MANAGER, NOTIFICATIONS, TERMINAL];

/// The general settings without the name of the NAS, which stays the one of the machine.
#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GeneralConfig {
    pub theme: Theme,
    #[serde(default)]
    pub themes: Themes,
    pub language: Language,
    pub create_shortcut: bool,
    #[serde(default)]
    pub desktop_icon_size: DesktopIconSize,
    #[serde(default)]
    pub accent: String,
    pub background: Background,
}

/// One shortcut of the desktop, in the order of the file.
#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DesktopEntry {
    pub kind: ShortcutKind,
    pub target: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// An application installed from a package. The version says what the NAS had: the import
/// installs the latest one.
#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InstalledApp {
    pub id: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AppManagerConfig {
    pub settings: ManagerSettings,
    /// The catalogue saved from the App Manager; none when the one of the installation applies.
    pub catalogue: Option<Vec<CatalogueApp>>,
    /// What the import installs or removes, so the NAS has these and no other. The system
    /// applications and the ones the installation brings are never removed.
    pub installed: Vec<InstalledApp>,
}

/// The settings of the applications chosen at the export, each under its identifier.
#[derive(Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AppsConfig {
    #[serde(
        rename = "nas-app-manager",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub app_manager: Option<AppManagerConfig>,
    #[serde(
        rename = "nas-notification-manager",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub notifications: Option<NotificationSettings>,
    #[serde(
        rename = "nas-system-terminal",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub terminal: Option<TerminalSettings>,
}

/// The configuration as the launcher exports and imports it.
#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Config {
    pub format: String,
    pub version: u32,
    /// The version of the launcher that exported it
    pub launcher: String,
    pub exported_at: i64,
    pub general: GeneralConfig,
    pub access: Access,
    pub access_rules: Vec<access::NewRule>,
    pub power: Power,
    pub widgets: WidgetSettings,
    /// Absent from a file of version 1: the desktop then stays as it is.
    #[serde(default)]
    pub desktop: Option<Vec<DesktopEntry>>,
    #[serde(default)]
    pub apps: AppsConfig,
}

/// The applications of `wanted`, each once, or why one cannot go in the file.
fn chosen(wanted: &[String]) -> Result<HashSet<&str>, ApiFailure> {
    wanted
        .iter()
        .map(|id| {
            EXPORTABLE
                .iter()
                .find(|exportable| **exportable == id.as_str())
                .copied()
                .ok_or_else(|| ApiFailure::bad(format!("{id} has no settings to export")))
        })
        .collect()
}

async fn export_app_manager(db: &SqlitePool) -> Result<AppManagerConfig, ApiFailure> {
    Ok(AppManagerConfig {
        settings: nas_app_manager::load_settings(db).await?,
        catalogue: catalogue::saved(db).await?.map(|saved| saved.apps),
        installed: nas_app_manager::installed(db)
            .await?
            .into_iter()
            .filter(|app| !app.system)
            .map(|app| InstalledApp {
                id: app.id,
                version: app.version,
            })
            .collect(),
    })
}

async fn export_apps(db: &SqlitePool, wanted: &[String]) -> Result<AppsConfig, ApiFailure> {
    let chosen = chosen(wanted)?;
    let mut apps = AppsConfig::default();
    if chosen.contains(APP_MANAGER) {
        apps.app_manager = Some(export_app_manager(db).await?);
    }
    if chosen.contains(NOTIFICATIONS) {
        apps.notifications =
            Some(crate::apps::services::nas_notification_manager::load_settings(db).await?);
    }
    if chosen.contains(TERMINAL) {
        apps.terminal = Some(crate::apps::services::nas_system_terminal::load_settings(db).await?);
    }
    Ok(apps)
}

/// The configuration of the NAS, with the settings of the applications in `apps`.
pub async fn export(db: &SqlitePool, apps: &[String]) -> Result<Config, ApiFailure> {
    let apps = export_apps(db, apps).await?;
    let general = super::load_general(db).await?;
    Ok(Config {
        format: FORMAT.to_owned(),
        version: VERSION,
        launcher: env!("CARGO_PKG_VERSION").to_owned(),
        exported_at: now_ms(),
        general: GeneralConfig {
            theme: general.theme,
            themes: general.themes,
            language: general.language,
            create_shortcut: general.create_shortcut,
            desktop_icon_size: general.desktop_icon_size,
            accent: general.accent,
            background: general.background,
        },
        access: super::load_access(db).await?,
        access_rules: access::list(db)
            .await?
            .into_iter()
            .map(|rule| access::NewRule {
                kind: rule.kind,
                value: rule.value,
                label: rule.label,
            })
            .collect(),
        power: super::load_power(db).await?,
        widgets: widgets::load(db).await?,
        desktop: Some(
            desktop::list(db)
                .await?
                .into_iter()
                .map(|shortcut| DesktopEntry {
                    kind: shortcut.kind,
                    target: shortcut.target,
                    name: shortcut.name,
                })
                .collect(),
        ),
        apps,
    })
}

/// The desktop of the file as rows, or why it is refused.
fn checked_desktop(
    entries: &[DesktopEntry],
) -> Result<Vec<(ShortcutKind, String, Option<String>)>, ApiFailure> {
    if entries.len() > MAX_SHORTCUTS {
        return Err(ApiFailure::bad("too many shortcuts"));
    }
    entries
        .iter()
        .map(|entry| match entry.kind {
            ShortcutKind::App if catalogue::is_identifier(&entry.target) => {
                Ok((ShortcutKind::App, entry.target.clone(), None))
            }
            ShortcutKind::App => Err(ApiFailure::bad("invalid shortcut target")),
            ShortcutKind::Folder => {
                desktop::validate_folder_path(&entry.target)?;
                let name = desktop::folder_name(entry.name.as_deref().unwrap_or_default())?;
                Ok((
                    ShortcutKind::Folder,
                    entry.target.clone(),
                    Some(name.to_owned()),
                ))
            }
        })
        .collect()
}

/// The part of the App Manager checked as its own saves check it: its catalogue, when it has one.
fn checked_app_manager(manager: &AppManagerConfig) -> Result<Option<Catalogue>, ApiFailure> {
    manager.settings.validate()?;
    if manager.installed.len() > MAX_INSTALLED
        || !manager
            .installed
            .iter()
            .all(|app| catalogue::is_identifier(&app.id) && !registry::is_system(&app.id))
    {
        return Err(ApiFailure::bad("invalid list of installed applications"));
    }
    manager
        .catalogue
        .clone()
        .map(catalogue::checked)
        .transpose()
}

/// What the file holds, checked before anything is written.
struct Checked {
    general: General,
    desktop: Option<Vec<(ShortcutKind, String, Option<String>)>>,
    catalogue: Option<Catalogue>,
}

/// Every part checked as its own save checks it, before anything is written: a refused file
/// changes nothing.
fn validate(config: &Config, name: String) -> Result<Checked, ApiFailure> {
    if config.format != FORMAT {
        return Err(ApiFailure::bad("not a configuration of nas-os"));
    }
    if config.version > VERSION {
        return Err(ApiFailure::bad(
            "the configuration comes from a newer launcher: update this one first",
        ));
    }
    if config.access_rules.len() > MAX_RULES {
        return Err(ApiFailure::bad("too many allowed addresses"));
    }
    let general = General {
        nas_name: name,
        theme: config.general.theme,
        themes: config.general.themes.clone(),
        language: config.general.language,
        create_shortcut: config.general.create_shortcut,
        desktop_icon_size: config.general.desktop_icon_size,
        accent: config.general.accent.clone(),
        background: config.general.background.clone(),
    };
    general.validate()?;
    config.power.validate()?;
    config.widgets.validate()?;
    for rule in &config.access_rules {
        access::checked(rule)?;
    }
    if let Some(settings) = &config.apps.notifications {
        settings.validate()?;
    }
    if let Some(settings) = &config.apps.terminal {
        settings.validate()?;
    }
    let catalogue = match &config.apps.app_manager {
        Some(manager) => checked_app_manager(manager)?,
        None => None,
    };
    Ok(Checked {
        general,
        desktop: config.desktop.as_deref().map(checked_desktop).transpose()?,
        catalogue,
    })
}

/// The applications on the NAS now, besides the system ones.
async fn external_ids(db: &SqlitePool) -> Result<Vec<String>, ApiFailure> {
    Ok(nas_app_manager::installed(db)
        .await?
        .into_iter()
        .filter(|app| !app.system)
        .map(|app| app.id)
        .collect())
}

/// The shortcuts of the file that point somewhere: a folder, or an application that is on the NAS
/// or that the import is about to install.
fn reachable(
    rows: Vec<(ShortcutKind, String, Option<String>)>,
    present: &[String],
    coming: Option<&AppManagerConfig>,
) -> Vec<(ShortcutKind, String, Option<String>)> {
    rows.into_iter()
        .filter(|(kind, target, _)| {
            *kind == ShortcutKind::Folder
                || registry::is_system(target)
                || present.contains(target)
                || coming
                    .is_some_and(|manager| manager.installed.iter().any(|app| &app.id == target))
        })
        .collect()
}

async fn write(
    db: &SqlitePool,
    config: &Config,
    checked: Checked,
    present: &[String],
) -> Result<(), ApiFailure> {
    let failed = |_| ApiFailure::unavailable("the configuration could not be saved");
    let mut transaction = db.begin().await.map_err(failed)?;
    settings::write(&mut *transaction, Section::General, &checked.general).await?;
    settings::write(&mut *transaction, Section::Access, &config.access).await?;
    settings::write(&mut *transaction, Section::Power, &config.power).await?;
    settings::write(&mut *transaction, Section::Widgets, &config.widgets).await?;
    sqlx::query("DELETE FROM access_rules")
        .execute(&mut *transaction)
        .await
        .map_err(failed)?;
    for rule in &config.access_rules {
        let (value, label) = access::checked(rule)?;
        sqlx::query(
            "INSERT INTO access_rules (kind, value, label, created_at) VALUES (?, ?, ?, ?)
             ON CONFLICT (kind, value) DO UPDATE SET label = excluded.label",
        )
        .bind(rule.kind.id())
        .bind(value)
        .bind(label)
        .bind(now_ms())
        .execute(&mut *transaction)
        .await
        .map_err(failed)?;
    }
    if let Some(rows) = checked.desktop {
        let rows = reachable(rows, present, config.apps.app_manager.as_ref());
        desktop::replace(&mut transaction, &rows).await?;
    }
    if let Some(settings) = &config.apps.notifications {
        settings::write(&mut *transaction, Section::Notifications, settings).await?;
    }
    if let Some(settings) = &config.apps.terminal {
        settings::write(&mut *transaction, Section::Terminal, settings).await?;
    }
    if let Some(manager) = &config.apps.app_manager {
        settings::write(&mut *transaction, Section::AppManager, &manager.settings).await?;
        settings::write(
            &mut *transaction,
            Section::Catalogue,
            &checked.catalogue.as_ref(),
        )
        .await?;
    }
    transaction.commit().await.map_err(failed)
}

/// What the import does to the installed applications.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub remove: Vec<String>,
    pub install: Vec<String>,
    /// In the file but not in the catalogue, so nowhere to install them from.
    pub missing: Vec<String>,
}

/// Installs what the file has and the NAS does not, and removes what the NAS has and the file does
/// not, but never a system application nor one the installation brings (`preinstall` in the
/// catalogue).
pub fn plan(present: &[String], wanted: &[InstalledApp], entries: &[CatalogueApp]) -> Plan {
    let in_file = |id: &str| wanted.iter().any(|app| app.id == id);
    let brought = |id: &str| {
        entries
            .iter()
            .any(|entry| entry.preinstall && entry.id == id)
    };
    let mut plan = Plan {
        remove: present
            .iter()
            .filter(|id| !in_file(id) && !brought(id) && !registry::is_system(id))
            .cloned()
            .collect(),
        ..Plan::default()
    };
    for app in wanted.iter().filter(|app| !present.contains(&app.id)) {
        if entries.iter().any(|entry| entry.id == app.id) {
            plan.install.push(app.id.clone());
        } else {
            plan.missing.push(app.id.clone());
        }
    }
    plan
}

/// Carries out the [`plan`] where packages are really installed: a development copy changes no
/// application. A failure is said in a notification and leaves the rest going.
async fn match_installed(db: &SqlitePool, wanted: &[InstalledApp], present: &[String]) {
    if !utils::is_real(Mode::Packages) {
        return;
    }
    let plan = plan(present, wanted, &catalogue::read(db).await);
    for id in plan.remove {
        if let Err(failure) = nas_app_manager::uninstall(db, id.clone(), false).await {
            eprintln!("{id} was not removed by the import: {}", failure.1);
        }
    }
    for id in plan.install {
        if let Err(failure) = nas_app_manager::install(db, id.clone()).await {
            eprintln!("{id} was not installed by the import: {}", failure.1);
        }
    }
    for id in plan.missing {
        notifications::record(
            db,
            "nas-app-manager",
            Tone::Warning,
            Notice::new(
                "configAppMissing",
                "Application not available",
                format!("{id} is in the configuration but not in the catalogue."),
            )
            .with("app", id),
        )
        .await;
    }
}

/// Replaces the configuration with the one of the file, all of it or nothing, then applies what
/// acts on the machine: the allowed addresses, the power schedule and the installed applications.
pub async fn import(db: &SqlitePool, config: Config) -> Result<Config, ApiFailure> {
    let current = super::load_general(db).await?;
    let checked = validate(&config, current.nas_name)?;
    let present = external_ids(db).await?;
    write(db, &config, checked, &present).await?;

    super::apply_screen(config.power.screen_off_minutes).await;
    schedule::rules_changed();
    access::apply_saved(db).await;
    if let Some(manager) = &config.apps.app_manager {
        match_installed(db, &manager.installed, &present).await;
    }
    notifications::record(
        db,
        "nas-system-settings",
        Tone::Success,
        Notice::new(
            "configImported",
            "Configuration imported",
            "NAS settings replaced with those of the imported file.",
        ),
    )
    .await;
    Ok(config)
}

#[cfg(test)]
#[path = "../../../../../tests/apps/services/nas_system_settings/config.rs"]
mod tests;
