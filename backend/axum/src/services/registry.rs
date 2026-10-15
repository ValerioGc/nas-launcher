//! Application registry.
//! It provides functions to query both system and installed applications.

use crate::utils::now_ms;
use sqlx::SqlitePool;

/// Description of a shell application that is always available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemApp {
    pub id: &'static str,
    pub name: &'static str,
    pub info: &'static str,
    metadata: &'static str,
}

impl SystemApp {
    pub fn version(&self) -> String {
        serde_json::from_str::<serde_json::Value>(self.metadata)
            .ok()
            .and_then(|metadata| metadata["version"].as_str().map(str::to_owned))
            .unwrap_or_default()
    }
}

/// Shell applications: always present and never installable or removable.
pub const SYSTEM: [SystemApp; 4] = [
    SystemApp {
        id: "nas-app-manager",
        name: "Application manager",
        info: "Lists, installs and removes the applications of the NAS.",
        metadata: include_str!("../../../../src/apps/nas-app-manager/metadata.json"),
    },
    SystemApp {
        id: "nas-global-search",
        name: "Global search",
        info: "Searches applications, settings and files.",
        metadata: include_str!("../../../../src/apps/nas-global-search/metadata.json"),
    },
    SystemApp {
        id: "nas-notification-manager",
        name: "Notifications",
        info: "Collects the notifications of the system and of the applications.",
        metadata: include_str!("../../../../src/apps/nas-notification-manager/metadata.json"),
    },
    SystemApp {
        id: "nas-system-settings",
        name: "Settings",
        info: "General settings, network, access, power and updates.",
        metadata: include_str!("../../../../src/apps/nas-system-settings/metadata.json"),
    },
];

/// Looks up an application compiled into the shell.
pub fn system(id: &str) -> Option<&'static SystemApp> {
    SYSTEM.iter().find(|app| app.id == id)
}

pub fn is_system(id: &str) -> bool {
    system(id).is_some()
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Installed {
    pub id: String,
    pub name: String,
    pub info: String,
    pub version: String,
}

/// The installed applications, in the order they were installed.
pub async fn installed(db: &SqlitePool) -> Result<Vec<Installed>, sqlx::Error> {
    sqlx::query_as("SELECT id, name, info, version FROM installed_apps ORDER BY installed_at, id")
        .fetch_all(db)
        .await
}

/// Records an installation, or updates the one already recorded.
pub async fn record(db: &SqlitePool, app: &Installed) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO installed_apps (id, name, info, version, installed_at) VALUES (?, ?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET
            name = excluded.name, info = excluded.info, version = excluded.version",
    )
    .bind(&app.id)
    .bind(&app.name)
    .bind(&app.info)
    .bind(&app.version)
    .bind(now_ms())
    .execute(db)
    .await
    .map(|_| ())
}

/// Records whether an installed application has work to do without its window; false when it
/// is not installed.
pub async fn set_background(db: &SqlitePool, id: &str, needed: bool) -> Result<bool, sqlx::Error> {
    sqlx::query("UPDATE installed_apps SET background = ? WHERE id = ?")
        .bind(needed)
        .bind(id)
        .execute(db)
        .await
        .map(|done| done.rows_affected() == 1)
}

/// The installed applications that said they have work to do without their window.
pub async fn in_background(db: &SqlitePool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM installed_apps WHERE background = 1 ORDER BY installed_at, id",
    )
    .fetch_all(db)
    .await
}

/// Removes the row of an application.
pub async fn forget(db: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM installed_apps WHERE id = ?")
        .bind(id)
        .execute(db)
        .await
        .map(|_| ())
}

#[cfg(test)]
#[path = "../../../tests/services/registry.rs"]
mod tests;
