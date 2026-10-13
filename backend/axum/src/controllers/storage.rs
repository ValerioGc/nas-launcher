use crate::{
    http::{error::ApiFailure, extract::ValidJson, state::AppState},
    services::storage::{
        self,
        eject::{self, EjectRequest, LockRequest},
        InitializeAccepted, InitializeRequest, RenameRequest, SmartRequest,
    },
};
use axum::{extract::State, Json};
use sqlx::SqlitePool;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Lists connected external disks and whether each can be safely ejected.
#[utoipa::path(
    get,
    path = "/storage/disks/external",
    responses((status = OK, body = Vec<eject::ExternalDisk>))
)]
async fn external_disks(
    State(db): State<SqlitePool>,
) -> Result<Json<Vec<eject::ExternalDisk>>, ApiFailure> {
    eject::external_disks(&db).await.map(Json)
}

/// Starts the constrained initialization of one empty data disk.
#[utoipa::path(
    post,
    path = "/storage/disks/initialize",
    request_body = InitializeRequest,
    responses((status = OK, body = InitializeAccepted))
)]
async fn initialize_disk(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<InitializeRequest>,
) -> Result<Json<InitializeAccepted>, ApiFailure> {
    storage::start_initialize(&db, input).await.map(Json)
}

/// Renames one writable ext data volume.
#[utoipa::path(
    post,
    path = "/storage/disks/rename",
    request_body = RenameRequest,
    responses((status = NO_CONTENT))
)]
async fn rename_disk(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<RenameRequest>,
) -> Result<axum::http::StatusCode, ApiFailure> {
    storage::rename_volume(&db, input).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/storage/disks/smart", request_body = SmartRequest)]
async fn smart_disk(
    ValidJson(input): ValidJson<SmartRequest>,
) -> Result<Json<serde_json::Value>, ApiFailure> {
    storage::smart_data(input).await.map(Json)
}

/// Ejects an external disk, unless it is marked as not removable.
#[utoipa::path(
    post,
    path = "/storage/disks/eject",
    request_body = EjectRequest,
    responses(
        (status = NO_CONTENT),
        (status = FORBIDDEN, description = "the disk is marked as not removable")
    )
)]
async fn eject_disk(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<EjectRequest>,
) -> Result<axum::http::StatusCode, ApiFailure> {
    eject::eject(&db, input).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// The filesystem UUIDs of the disks marked as not removable.
#[utoipa::path(get, path = "/storage/locks", responses((status = OK, body = Vec<String>)))]
async fn locks(State(db): State<SqlitePool>) -> Result<Json<Vec<String>>, ApiFailure> {
    eject::locks(&db).await.map(Json)
}

/// Marks a disk as not removable, or removable again.
#[utoipa::path(
    post,
    path = "/storage/locks",
    request_body = LockRequest,
    responses((status = OK, body = Vec<String>))
)]
async fn set_lock(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<LockRequest>,
) -> Result<Json<Vec<String>>, ApiFailure> {
    eject::set_lock(&db, input).await.map(Json)
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(external_disks))
        .routes(routes!(initialize_disk))
        .routes(routes!(rename_disk))
        .routes(routes!(smart_disk))
        .routes(routes!(eject_disk))
        .routes(routes!(locks, set_lock))
}
