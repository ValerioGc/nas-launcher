//! Desktop shortcuts stored in the database

use crate::{
    apps::services::nas_app_manager,
    http::error::ApiFailure,
    services::settings::{self, Section},
    utils::now_ms,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

const LOCAL_USER: &str = "local";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ShortcutKind {
    App,
    Folder,
}
impl ShortcutKind {
    fn id(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Folder => "folder",
        }
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Shortcut {
    pub kind: ShortcutKind,
    pub target: String,
    pub name: Option<String>,
    pub position: i64,
}

/// The applications on the desktop in the order they are shown, first to last.
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Order {
    pub order: Vec<ShortcutOrder>,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(untagged)]
pub enum ShortcutOrder {
    App(String),
    Key(ShortcutKey),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ShortcutKey {
    pub kind: ShortcutKind,
    pub target: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FolderShortcut {
    pub path: String,
    pub name: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FolderPath {
    pub path: String,
}

/// A new application gets its shortcut unless the general settings say otherwise.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopPreferences {
    #[serde(default = "shortcut_by_default")]
    create_shortcut: bool,
}

const fn shortcut_by_default() -> bool {
    true
}

impl Default for DesktopPreferences {
    fn default() -> Self {
        Self {
            create_shortcut: shortcut_by_default(),
        }
    }
}

pub async fn shortcut_on_install(db: &SqlitePool) -> bool {
    settings::read::<DesktopPreferences>(db, Section::General)
        .await
        .map_or(shortcut_by_default(), |preferences| {
            preferences.create_shortcut
        })
}

pub async fn list(db: &SqlitePool) -> Result<Vec<Shortcut>, ApiFailure> {
    let rows: Vec<(String, String, Option<String>, i64)> = sqlx::query_as(
        "SELECT kind, target, name, position FROM desktop_shortcuts
         WHERE user_name = ? ORDER BY position, target",
    )
    .bind(LOCAL_USER)
    .fetch_all(db)
    .await
    .map_err(|_| unavailable())?;
    Ok(rows
        .into_iter()
        .map(|(kind, target, name, position)| Shortcut {
            kind: if kind == "folder" {
                ShortcutKind::Folder
            } else {
                ShortcutKind::App
            },
            target,
            name,
            position,
        })
        .collect())
}

pub async fn add_app(db: &SqlitePool, id: &str) -> Result<Vec<Shortcut>, ApiFailure> {
    if !nas_app_manager::is_installed(db, id).await? {
        return Err(ApiFailure::missing("installed application"));
    }
    let next: (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM desktop_shortcuts WHERE user_name = ?",
    )
    .bind(LOCAL_USER)
    .fetch_one(db)
    .await
    .map_err(|_| unavailable())?;
    sqlx::query(
        "INSERT INTO desktop_shortcuts (user_name, kind, target, name, position, created_at)
         VALUES (?, ?, ?, NULL, ?, ?)
         ON CONFLICT (user_name, kind, target) DO NOTHING",
    )
    .bind(LOCAL_USER)
    .bind(ShortcutKind::App.id())
    .bind(id)
    .bind(next.0)
    .bind(now_ms())
    .execute(db)
    .await
    .map_err(|_| unavailable())?;
    list(db).await
}

pub async fn add_folder(
    db: &SqlitePool,
    input: FolderShortcut,
) -> Result<Vec<Shortcut>, ApiFailure> {
    validate_folder_path(&input.path)?;
    let name = folder_name(&input.name)?;
    if !nas_app_manager::is_installed(db, "nas-file-manager").await? {
        return Err(ApiFailure::missing("installed application"));
    }
    let next: (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM desktop_shortcuts WHERE user_name = ?",
    )
    .bind(LOCAL_USER)
    .fetch_one(db)
    .await
    .map_err(|_| unavailable())?;
    sqlx::query(
        "INSERT INTO desktop_shortcuts (user_name, kind, target, name, position, created_at)
         VALUES (?, 'folder', ?, ?, ?, ?)
         ON CONFLICT (user_name, kind, target) DO NOTHING",
    )
    .bind(LOCAL_USER)
    .bind(&input.path)
    .bind(name)
    .bind(next.0)
    .bind(now_ms())
    .execute(db)
    .await
    .map_err(|_| unavailable())?;
    list(db).await
}

/// The name of a folder shortcut, trimmed, or why it is refused.
pub fn folder_name(name: &str) -> Result<&str, ApiFailure> {
    let name = name.trim();
    if name.is_empty() || name.len() > 160 || name.chars().any(char::is_control) {
        return Err(ApiFailure::bad("invalid folder shortcut name"));
    }
    Ok(name)
}

pub fn validate_folder_path(path: &str) -> Result<(), ApiFailure> {
    let mut location = url::form_urlencoded::Serializer::new(String::new());
    location.append_pair("path", path);
    if !path.starts_with('/')
        || path.starts_with("//")
        || path.contains("//")
        || location.finish().len() + 1 > 2048
        || path.chars().any(char::is_control)
        || path.contains('\\')
        || path.split('/').any(|part| part == "." || part == "..")
    {
        return Err(ApiFailure::bad("invalid folder shortcut path"));
    }
    Ok(())
}

pub async fn remove(
    db: &SqlitePool,
    kind: ShortcutKind,
    target: &str,
) -> Result<Vec<Shortcut>, ApiFailure> {
    let removed = sqlx::query(
        "DELETE FROM desktop_shortcuts WHERE user_name = ? AND kind = ? AND target = ?",
    )
    .bind(LOCAL_USER)
    .bind(kind.id())
    .bind(target)
    .execute(db)
    .await
    .map_err(|_| unavailable())?;
    if removed.rows_affected() == 0 {
        return Err(ApiFailure::missing("shortcut"));
    }
    list(db).await
}

pub async fn remove_folder(db: &SqlitePool, path: &str) -> Result<Vec<Shortcut>, ApiFailure> {
    validate_folder_path(path)?;
    remove(db, ShortcutKind::Folder, path).await
}

/// The key of one entry of an order, or why it is refused.
fn order_key(item: ShortcutOrder) -> Result<ShortcutKey, ApiFailure> {
    let key = match item {
        ShortcutOrder::App(target) => ShortcutKey {
            kind: ShortcutKind::App,
            target,
        },
        ShortcutOrder::Key(key) => key,
    };
    let limit = if key.kind == ShortcutKind::App {
        64
    } else {
        1800
    };
    if key.target.is_empty() || key.target.len() > limit {
        return Err(ApiFailure::bad("invalid shortcut target"));
    }
    Ok(key)
}

/// The shortcuts in the order asked, once each and only those that exist, then the ones the order
/// left out, in their current order.
fn placement(
    current: &[Shortcut],
    order: Vec<ShortcutOrder>,
) -> Result<Vec<ShortcutKey>, ApiFailure> {
    let saved: Vec<ShortcutKey> = current
        .iter()
        .map(|shortcut| ShortcutKey {
            kind: shortcut.kind,
            target: shortcut.target.clone(),
        })
        .collect();
    let mut placed: Vec<ShortcutKey> = Vec::new();
    for item in order {
        let key = order_key(item)?;
        if saved.contains(&key) && !placed.contains(&key) {
            placed.push(key);
        }
    }
    for key in saved {
        if !placed.contains(&key) {
            placed.push(key);
        }
    }
    Ok(placed)
}

/// Moves the desktop shortcuts into the given order. The ones the order leaves out keep their
/// relative place after the others, so a list read before another change still applies.
pub async fn reorder(db: &SqlitePool, order: Order) -> Result<Vec<Shortcut>, ApiFailure> {
    if order.order.len() > 500 {
        return Err(ApiFailure::bad("too many shortcuts"));
    }
    let placed = placement(&list(db).await?, order.order)?;
    let mut transaction = db.begin().await.map_err(|_| unavailable())?;
    for (position, key) in placed.iter().enumerate() {
        sqlx::query(
            "UPDATE desktop_shortcuts SET position = ?
             WHERE user_name = ? AND kind = ? AND target = ?",
        )
        .bind(i64::try_from(position).unwrap_or(i64::MAX))
        .bind(LOCAL_USER)
        .bind(key.kind.id())
        .bind(&key.target)
        .execute(&mut *transaction)
        .await
        .map_err(|_| unavailable())?;
    }
    transaction.commit().await.map_err(|_| unavailable())?;
    list(db).await
}

/// Puts `shortcuts` on the desktop in place of the ones there, in their order, inside the
/// transaction of an import. They were checked by the caller.
pub async fn replace(
    connection: &mut sqlx::SqliteConnection,
    shortcuts: &[(ShortcutKind, String, Option<String>)],
) -> Result<(), ApiFailure> {
    sqlx::query("DELETE FROM desktop_shortcuts WHERE user_name = ?")
        .bind(LOCAL_USER)
        .execute(&mut *connection)
        .await
        .map_err(|_| unavailable())?;
    for (position, (kind, target, name)) in shortcuts.iter().enumerate() {
        sqlx::query(
            "INSERT INTO desktop_shortcuts (user_name, kind, target, name, position, created_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (user_name, kind, target) DO NOTHING",
        )
        .bind(LOCAL_USER)
        .bind(kind.id())
        .bind(target)
        .bind(name)
        .bind(i64::try_from(position).unwrap_or(i64::MAX))
        .bind(now_ms())
        .execute(&mut *connection)
        .await
        .map_err(|_| unavailable())?;
    }
    Ok(())
}

pub async fn forget_app(db: &SqlitePool, id: &str) {
    let _ = sqlx::query(
        "DELETE FROM desktop_shortcuts WHERE user_name = ? AND kind = ? AND target = ?",
    )
    .bind(LOCAL_USER)
    .bind(ShortcutKind::App.id())
    .bind(id)
    .execute(db)
    .await;
}

fn unavailable() -> ApiFailure {
    ApiFailure::unavailable("desktop shortcuts unavailable")
}

#[cfg(test)]
#[path = "../../tests/services/desktop.rs"]
mod tests;
