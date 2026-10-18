use crate::{http::error::ApiFailure, services::packages::catalogue, utils::now_ms};
use sqlx::SqlitePool;
use std::{collections::HashMap, path::PathBuf};

pub const MAX_BYTES: usize = 256 * 1024;

// Allowed formats for logos, with their MIME types.
const FORMATS: [(&str, &str); 4] = [
    ("svg", "image/svg+xml"),
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("webp", "image/webp"),
];

/// Uses `NAS_LOGOS_DIR`, then `/usr/local/share/nas-os/logos`.
fn defaults_directory() -> PathBuf {
    if let Ok(configured) = std::env::var("NAS_LOGOS_DIR") {
        return PathBuf::from(configured);
    }
    let installed = PathBuf::from("/usr/local/share/nas-os/logos");
    if installed.exists() || !cfg!(debug_assertions) {
        return installed;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../system/packages/ui_installation/files/logos")
}

pub fn format(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return Some("image/jpeg");
    }
    if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let text = text.trim_start_matches('\u{feff}').trim_start();
    (text.starts_with('<') && text.contains("<svg")).then_some("image/svg+xml")
}

/// Replaces the logo of an application of the catalogue.
pub async fn store(db: &SqlitePool, id: &str, bytes: &[u8]) -> Result<(), ApiFailure> {
    if catalogue::find(db, id).await.is_none() {
        return Err(ApiFailure::missing(format!("unknown application: {id}")));
    }
    if bytes.len() > MAX_BYTES {
        return Err(ApiFailure::bad("the logo is larger than 256 KiB"));
    }
    let mime = format(bytes)
        .ok_or_else(|| ApiFailure::bad("the logo is not an SVG, PNG, JPEG or WebP image"))?;
    sqlx::query(
        "INSERT INTO app_logos (id, mime, data, updated_at) VALUES (?, ?, ?, ?)
         ON CONFLICT (id) DO UPDATE SET mime = excluded.mime, data = excluded.data,
         updated_at = excluded.updated_at",
    )
    .bind(id)
    .bind(mime)
    .bind(bytes)
    .bind(now_ms())
    .execute(db)
    .await
    .map_err(|_| unavailable())?;
    Ok(())
}

pub async fn forget(db: &SqlitePool, id: &str) -> Result<(), ApiFailure> {
    sqlx::query("DELETE FROM app_logos WHERE id = ?")
        .bind(id)
        .execute(db)
        .await
        .map_err(|_| unavailable())?;
    Ok(())
}

pub async fn keep_only(db: &SqlitePool, ids: &[&str]) -> Result<(), ApiFailure> {
    let stored: Vec<(String,)> = sqlx::query_as("SELECT id FROM app_logos")
        .fetch_all(db)
        .await
        .map_err(|_| unavailable())?;
    for (id,) in stored {
        if !ids.contains(&id.as_str()) {
            forget(db, &id).await?;
        }
    }
    Ok(())
}

pub async fn read(db: &SqlitePool, id: &str) -> Result<(String, Vec<u8>), ApiFailure> {
    let missing = || ApiFailure::missing(format!("{id} has no logo"));
    if !catalogue::is_identifier(id) {
        return Err(missing());
    }
    let uploaded: Option<(String, Vec<u8>)> =
        sqlx::query_as("SELECT mime, data FROM app_logos WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await
            .map_err(|_| unavailable())?;
    if let Some(found) = uploaded {
        return Ok(found);
    }

    let (file, mime) = on_machine(id).await.ok_or_else(missing)?;
    let bytes = tokio::fs::read(file)
        .await
        .map_err(|error| ApiFailure::unavailable(format!("cannot read the logo: {error}")))?;
    Ok((mime.to_owned(), bytes))
}

pub async fn addresses(db: &SqlitePool) -> HashMap<String, String> {
    let mut found = HashMap::new();
    if let Ok(mut directory) = tokio::fs::read_dir(defaults_directory()).await {
        while let Ok(Some(entry)) = directory.next_entry().await {
            let path = entry.path();
            let known = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| FORMATS.iter().any(|(known, _)| *known == extension));
            if let (true, Some(id)) = (known, path.file_stem().and_then(|stem| stem.to_str())) {
                found.insert(id.to_owned(), address(id, None));
            }
        }
    }
    let uploaded: Vec<(String, i64)> = sqlx::query_as("SELECT id, updated_at FROM app_logos")
        .fetch_all(db)
        .await
        .unwrap_or_default();
    for (id, updated_at) in uploaded {
        let url = address(&id, Some(updated_at));
        found.insert(id, url);
    }
    found
}

fn address(id: &str, version: Option<i64>) -> String {
    match version {
        Some(version) => format!("/api/apps/{id}/logo?v={version}"),
        None => format!("/api/apps/{id}/logo"),
    }
}

async fn on_machine(id: &str) -> Option<(PathBuf, &'static str)> {
    let directory = defaults_directory();
    for (extension, mime) in FORMATS {
        let file = directory.join(format!("{id}.{extension}"));
        if tokio::fs::metadata(&file)
            .await
            .is_ok_and(|found| found.is_file() && found.len() <= MAX_BYTES as u64)
        {
            return Some((file, mime));
        }
    }
    None
}

fn unavailable() -> ApiFailure {
    ApiFailure::unavailable("application logos unavailable")
}

#[cfg(test)]
#[path = "../../../tests/services/packages/logos.rs"]
mod tests;
