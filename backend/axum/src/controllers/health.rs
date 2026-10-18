use crate::{
    http::{error::ApiFailure, state::AppState},
    services::registry,
};
use axum::{extract::State, Json};
use serde::Serialize;
use sqlx::SqlitePool;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Service state returned by the health check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Health {
    pub status: &'static str,
    pub version: &'static str,
}

#[utoipa::path(get, path = "/health", responses((status = OK, body = Health)))]
pub async fn health(State(db): State<SqlitePool>) -> Result<Json<Health>, ApiFailure> {
    sqlx::query("SELECT 1")
        .execute(&db)
        .await
        .map_err(|_| ApiFailure::unavailable("database unavailable"))?;
    Ok(Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(Serialize, ToSchema)]
struct BackendStatus {
    service: &'static str,
    version: &'static str,
    online: bool,
    system_apps: usize,
}

/// Service name and version and the number of system applications
#[utoipa::path(get, path = "/status", responses((status = OK, body = BackendStatus)))]
async fn status() -> Json<BackendStatus> {
    Json(BackendStatus {
        service: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        online: true,
        system_apps: registry::SYSTEM.len(),
    })
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(status))
}

/// The health check served outside the API prefix
pub fn health_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(health))
}

pub async fn not_found() -> ApiFailure {
    ApiFailure::missing("resource not found")
}
