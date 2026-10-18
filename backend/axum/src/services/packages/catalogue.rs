//! The installable applications catalogue.
//! It defines the structure of the catalogue and provides functions to validate and access it.

use crate::{
    http::error::ApiFailure,
    services::{
        registry,
        settings::{self, Section},
    },
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{collections::HashSet, path::PathBuf};

const MAX_APPS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(as = ForgeProvider)]
pub enum Provider {
    Github,
    Gitlab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Media,
    Storage,
    Network,
    #[serde(alias = "files", alias = "security", alias = "utilities")]
    System,
    Development,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CatalogueApp {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub system: bool,
    #[serde(default)]
    pub info: String,
    pub provider: Provider,
    pub repository: String,
    pub asset: String,
    #[serde(default, skip_serializing)]
    pub private: bool,
    #[serde(default)]
    pub preinstall: bool,
    #[serde(default, skip_serializing)]
    pub logo: String,
    #[serde(default)]
    pub category: Option<Category>,
}

pub fn is_identifier(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
}

impl CatalogueApp {
    fn valid(&self) -> bool {
        let word = |value: &str| {
            !matches!(value, "" | "." | "..")
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        };
        let id = is_identifier(&self.id) && !registry::is_system(&self.id);
        let repository = self.repository.len() <= 200
            && self.repository.split('/').count() >= 2
            && self.repository.split('/').all(word);
        let asset = (1..=200).contains(&self.asset.len())
            && !self.asset.contains(['/', '\\'])
            && !self.asset.chars().any(char::is_whitespace);
        id && repository
            && asset
            && (1..=60).contains(&self.name.trim().chars().count())
            && self.info.chars().count() <= 300
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    pub version: u32,
    #[serde(default, skip_serializing)]
    pub credential: String,
    #[serde(default, deserialize_with = "deserialize_apps")]
    pub apps: Vec<CatalogueApp>,
}

// Keep saved and installed catalogues readable, including Explorer's former Files default.
fn deserialize_apps<'de, D>(deserializer: D) -> Result<Vec<CatalogueApp>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let mut apps = Vec::<serde_json::Value>::deserialize(deserializer)?;
    for app in &mut apps {
        if app["id"] == "nas-file-manager" && app["category"] == "files" {
            app["category"] = serde_json::Value::String("storage".to_owned());
        }
    }
    apps.into_iter()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .map_err(serde::de::Error::custom)
}

/// Uses `NAS_CATALOGUE_PATH`, then `/etc/nas-os/catalogue.json`
pub fn path() -> PathBuf {
    if let Ok(configured) = std::env::var("NAS_CATALOGUE_PATH") {
        return PathBuf::from(configured);
    }

    let installed = PathBuf::from("/etc/nas-os/catalogue.json");
    if installed.exists() || !cfg!(debug_assertions) {
        return installed;
    }

    if let Some(beside) =
        crate::services::database::beside("catalogue.json").filter(|path| path.exists())
    {
        return beside;
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../system/packages/ui_installation/files/catalogue.json")
}

pub async fn read_config(db: &SqlitePool) -> Catalogue {
    match settings::read::<Option<Catalogue>>(db, Section::Catalogue).await {
        Ok(Some(saved)) => saved,
        _ => read_file().await,
    }
}

/// The catalogue of `apps`, or why it is refused.
pub fn checked(apps: Vec<CatalogueApp>) -> Result<Catalogue, ApiFailure> {
    if apps.len() > MAX_APPS || !apps.iter().all(CatalogueApp::valid) {
        return Err(ApiFailure::bad("invalid catalogue entry"));
    }
    let mut ids = HashSet::new();
    if !apps.iter().all(|app| ids.insert(app.id.as_str())) {
        return Err(ApiFailure::bad("two catalogue entries share an identifier"));
    }
    Ok(Catalogue {
        version: 1,
        credential: String::new(),
        apps,
    })
}

pub async fn save(db: &SqlitePool, apps: Vec<CatalogueApp>) -> Result<Catalogue, ApiFailure> {
    let catalogue = checked(apps)?;
    settings::write(db, Section::Catalogue, &Some(&catalogue)).await?;
    Ok(catalogue)
}

/// The catalogue saved from the App Manager, or nothing when the one of the installation applies.
pub async fn saved(db: &SqlitePool) -> Result<Option<Catalogue>, ApiFailure> {
    settings::read::<Option<Catalogue>>(db, Section::Catalogue).await
}

async fn read_file() -> Catalogue {
    let file = path();
    let Ok(text) = tokio::fs::read_to_string(&file).await else {
        return Catalogue::default();
    };

    match serde_json::from_str::<Catalogue>(&text) {
        Ok(catalogue) if catalogue.version == 1 => catalogue,
        Ok(catalogue) => {
            eprintln!(
                "catalogue {} declares version {}, which this launcher does not read",
                file.display(),
                catalogue.version
            );
            Catalogue::default()
        }
        Err(error) => {
            eprintln!("unreadable catalogue {}: {error}", file.display());
            Catalogue::default()
        }
    }
}

pub async fn find(db: &SqlitePool, id: &str) -> Option<CatalogueApp> {
    read(db).await.into_iter().find(|app| app.id == id)
}

pub async fn read(db: &SqlitePool) -> Vec<CatalogueApp> {
    read_config(db).await.apps
}

#[cfg(test)]
#[path = "../../../tests/services/packages/catalogue.rs"]
mod tests;
