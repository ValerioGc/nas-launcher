use crate::{
    apps::services::antivirus::{self, EntryRequest, ScanRequest, Settings, Status, Toggle},
    http::{error::ApiFailure, extract::ValidJson, state::AppState},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sqlx::SqlitePool;
use utoipa_axum::{router::OpenApiRouter, routes};

#[utoipa::path(get, path = "/antivirus", responses((status = OK, body = Status)))]
async fn status(State(db): State<SqlitePool>) -> Result<Json<Status>, ApiFailure> {
    antivirus::status(&db).await.map(Json)
}
#[utoipa::path(put, path = "/antivirus/settings", request_body = Settings, responses((status = OK, body = Settings)))]
async fn save(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<Settings>,
) -> Result<Json<Settings>, ApiFailure> {
    antivirus::save(&db, input).await.map(Json)
}
#[utoipa::path(post, path = "/antivirus/enabled", request_body = Toggle, responses((status = NO_CONTENT)))]
async fn toggle(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<Toggle>,
) -> Result<StatusCode, ApiFailure> {
    antivirus::toggle(&db, input.enabled).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post, path = "/antivirus/scan", request_body = ScanRequest, responses((status = NO_CONTENT)))]
async fn scan(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<ScanRequest>,
) -> Result<StatusCode, ApiFailure> {
    antivirus::scan(&db, input).await?;
    Ok(StatusCode::NO_CONTENT)
}
/// Forgets the files found clean, which the quick scan leaves out while they do not change
#[utoipa::path(post, path = "/antivirus/scanned/forget", responses((status = NO_CONTENT)))]
async fn forget(State(db): State<SqlitePool>) -> Result<StatusCode, ApiFailure> {
    antivirus::forget_scanned(&db).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[utoipa::path(post, path = "/antivirus/quarantine/{action}", request_body = EntryRequest, responses((status = NO_CONTENT)))]
async fn quarantine(
    State(db): State<SqlitePool>,
    Path(action): Path<String>,
    ValidJson(input): ValidJson<EntryRequest>,
) -> Result<StatusCode, ApiFailure> {
    antivirus::quarantine_action(&db, &action, input.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(status))
        .routes(routes!(save))
        .routes(routes!(toggle))
        .routes(routes!(scan))
        .routes(routes!(quarantine))
        .routes(routes!(forget))
}
