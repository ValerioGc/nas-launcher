//! Application caches and trash exposed to the Cleanup app.
//!
//! Applications declare a versioned `cache` capability with an internal endpoint.
//! GET lists cache entries and DELETE clears an entry by ID. Internal endpoints
//! are accessible only to the launcher, which starts applications when needed.
//! The list also includes the antivirus clean-file cache and application trash.

use crate::{
    apps::services::antivirus,
    http::error::ApiFailure,
    services::{
        mail_events::{call, is_internal_endpoint},
        manifest::{self, Manifest},
    },
};
use axum::http::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

pub const CAPABILITY: &str = "cache";
/// Trash endpoint provided by nas-file-manager.
/// GET returns its label and size. DELETE permanently removes its contents.
pub const TRASH: &str = "trash";
/// The application the archive of the antivirus scans is shown under, and its id.
const ANTIVIRUS: (&str, &str) = ("nas-security-center", "antivirus-scans");

/// A cache, made again when emptied, or a trash, whose content goes for good.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    #[default]
    Cache,
    Trash,
}

/// One cache, with the application it belongs to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Cache {
    #[serde(default)]
    pub app: String,
    pub id: String,
    pub label: String,
    pub bytes: u64,
    pub items: u64,
    #[serde(default)]
    pub kind: Kind,
}

/// Application trash response.
#[derive(Deserialize)]
struct Trash {
    label: String,
    bytes: u64,
    items: u64,
}

#[derive(Deserialize)]
struct Declared {
    caches: Vec<Cache>,
}

/// The applications that declare caches, with their endpoint.
pub fn holders(manifests: &[Manifest]) -> Vec<(&Manifest, &str)> {
    holders_of(manifests, CAPABILITY)
}

/// The applications that declare a capability of this kind, with their internal endpoint.
pub fn holders_of<'a>(manifests: &'a [Manifest], capability: &str) -> Vec<(&'a Manifest, &'a str)> {
    manifests
        .iter()
        .filter_map(|manifest| {
            manifest
                .provides
                .iter()
                .find(|offered| {
                    offered.capability == capability
                        && offered.version == 1
                        && is_internal_endpoint(&offered.endpoint)
                })
                .map(|offered| (manifest, offered.endpoint.as_str()))
        })
        .collect()
}

/// Application cache response with IDs scoped to that application.
pub fn declared(app: &str, body: &[u8]) -> Vec<Cache> {
    serde_json::from_slice::<Declared>(body)
        .map(|declared| declared.caches)
        .unwrap_or_default()
        .into_iter()
        .filter(|cache| manifest::is_identifier(&cache.id) && cache.label.len() <= 120)
        .map(|cache| Cache {
            app: app.to_owned(),
            kind: Kind::Cache,
            ..cache
        })
        .collect()
}

/// The archive of the antivirus scans, when it holds something.
fn antivirus_cache(report: &antivirus::Report) -> Option<Cache> {
    (report.cache_files > 0 || report.cache_bytes > 0).then(|| Cache {
        app: ANTIVIRUS.0.to_owned(),
        id: ANTIVIRUS.1.to_owned(),
        label: "Archivio delle scansioni antivirus".to_owned(),
        bytes: report.cache_bytes,
        items: report.cache_files,
        kind: Kind::Cache,
    })
}

/// Every cache of the NAS. An application that does not answer is left out.
pub async fn list() -> Vec<Cache> {
    let manifests = manifest::all().await;
    let mut caches = Vec::new();
    for (app, endpoint) in holders(&manifests) {
        match call(app, Method::GET, endpoint, Vec::new()).await {
            Ok((status, body)) if status.is_success() => caches.extend(declared(&app.id, &body)),
            Ok((status, _)) => eprintln!("the caches of {} were not read: {status}", app.id),
            Err(reason) => eprintln!("the caches of {} were not read: {reason}", app.id),
        }
    }
    for (app, endpoint) in holders_of(&manifests, TRASH) {
        match call(app, Method::GET, endpoint, Vec::new()).await {
            Ok((status, body)) if status.is_success() => caches.extend(trash_of(&app.id, &body)),
            Ok((status, _)) => eprintln!("the trash of {} was not read: {status}", app.id),
            Err(reason) => eprintln!("the trash of {} was not read: {reason}", app.id),
        }
    }
    caches.extend(antivirus_cache(&antivirus::report().await));
    caches
}

/// Converts application trash metadata to a cache-list entry.
pub fn trash_of(app: &str, body: &[u8]) -> Option<Cache> {
    let trash: Trash = serde_json::from_slice(body).ok()?;
    (trash.label.len() <= 120).then(|| Cache {
        app: app.to_owned(),
        id: TRASH.to_owned(),
        label: trash.label,
        bytes: trash.bytes,
        items: trash.items,
        kind: Kind::Trash,
    })
}

/// Empties one cache.
pub async fn clear(db: &SqlitePool, app: &str, id: &str) -> Result<(), ApiFailure> {
    if !manifest::is_identifier(app) || !manifest::is_identifier(id) {
        return Err(ApiFailure::bad("invalid cache"));
    }
    if (app, id) == ANTIVIRUS {
        return antivirus::forget_scanned(db).await;
    }
    let manifests = manifest::all().await;
    let trash = holders_of(&manifests, TRASH)
        .into_iter()
        .find(|(manifest, _)| manifest.id == app && id == TRASH);
    let (holder, path) = match trash {
        Some((holder, endpoint)) => (holder, endpoint.to_owned()),
        None => holders(&manifests)
            .into_iter()
            .find(|(manifest, _)| manifest.id == app)
            .map(|(holder, endpoint)| (holder, format!("{}/{id}", endpoint.trim_end_matches('/'))))
            .ok_or_else(|| ApiFailure::missing("cache"))?,
    };
    let (status, _) = call(holder, Method::DELETE, &path, Vec::new())
        .await
        .map_err(ApiFailure::unavailable)?;
    match status {
        status if status.is_success() => Ok(()),
        StatusCode::NOT_FOUND => Err(ApiFailure::missing("cache")),
        status => Err(ApiFailure(
            StatusCode::BAD_GATEWAY,
            format!("{app} answered {status}"),
        )),
    }
}

#[cfg(test)]
#[path = "../../tests/services/caches.rs"]
mod tests;
