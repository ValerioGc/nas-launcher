use crate::{
    http::{error::ApiFailure, state::AppState},
    services::caches::{self, Cache},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sqlx::SqlitePool;
use utoipa_axum::{router::OpenApiRouter, routes};

/// The caches of the NAS: those the applications declare, and the archive of the antivirus scans
#[utoipa::path(get, path = "/caches", responses((status = OK, body = Vec<Cache>)))]
async fn list() -> Json<Vec<Cache>> {
    Json(caches::list().await)
}

/// Empties a cache; it is made again from the data
#[utoipa::path(
    delete,
    path = "/caches/{app}/{id}",
    params(
        ("app" = String, Path, description = "the application the cache belongs to"),
        ("id" = String, Path, description = "the cache")
    ),
    responses((status = NO_CONTENT))
)]
async fn clear(
    State(db): State<SqlitePool>,
    Path((app, id)): Path<(String, String)>,
) -> Result<StatusCode, ApiFailure> {
    caches::clear(&db, &app, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(clear))
}

#[cfg(test)]
#[path = "../../../tests/controllers/caches.rs"]
mod tests;
