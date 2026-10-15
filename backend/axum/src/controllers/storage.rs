use crate::{
    http::{error::ApiFailure, extract::ValidJson, state::AppState},
    services::storage::{
        self,
        crypt::{self, Answer, Request},
        eject::{self, EjectRequest, LockRequest},
        inventory::{self, Disk},
        network::{self, NetworkDisks, NetworkRequest},
        InitializeAccepted, InitializeRequest, RenameRequest, SmartRequest,
    },
};
use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Deserialize, utoipa::IntoParams)]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
struct DiskQuery {
    /// `local` or `external`; every disk when missing
    kind: Option<String>,
}

/// The disks and volumes of the machine as the disk manager shows them, the system left out.
#[utoipa::path(
    get,
    path = "/storage/disks",
    params(DiskQuery),
    responses((status = OK, body = Vec<Disk>))
)]
async fn disks(Query(query): Query<DiskQuery>) -> Result<Json<Vec<Disk>>, ApiFailure> {
    if query
        .kind
        .as_deref()
        .is_some_and(|kind| !matches!(kind, "local" | "external"))
    {
        return Err(ApiFailure::bad("invalid disk kind"));
    }
    tokio::task::spawn_blocking(move || inventory::disks(query.kind.as_deref()))
        .await
        .map_err(|_| ApiFailure::unavailable("disk inspection failed"))?
        .map(Json)
        .map_err(ApiFailure::unavailable)
}

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

/// Whether disk encryption is on for this installation.
#[utoipa::path(get, path = "/storage/encryption", responses((status = OK, body = bool)))]
async fn encryption() -> Json<bool> {
    Json(crypt::available())
}

/// Formats a data disk, or opens, closes and keys an encrypted volume.
#[utoipa::path(
    post,
    path = "/storage/encryption",
    request_body = Request,
    responses((status = OK, body = Answer))
)]
async fn encryption_request(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<Request>,
) -> Result<Json<Answer>, ApiFailure> {
    crypt::request(&db, input).await.map(Json)
}

/// The network disks the NAS mounts, connected or not.
#[utoipa::path(get, path = "/storage/network", responses((status = OK, body = NetworkDisks)))]
async fn network_disks() -> Result<Json<NetworkDisks>, ApiFailure> {
    tokio::task::spawn_blocking(network::list)
        .await
        .map_err(|_| ApiFailure::unavailable("network disks unavailable"))?
        .map(Json)
        .map_err(ApiFailure::unavailable)
}

/// Adds, tries, connects, disconnects or forgets a network disk.
#[utoipa::path(
    post,
    path = "/storage/network",
    request_body = NetworkRequest,
    responses((status = OK, body = Answer))
)]
async fn network_request(
    ValidJson(input): ValidJson<NetworkRequest>,
) -> Result<Json<Answer>, ApiFailure> {
    network::request(input).await.map(Json)
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(disks))
        .routes(routes!(encryption, encryption_request))
        .routes(routes!(network_disks, network_request))
        .routes(routes!(external_disks))
        .routes(routes!(initialize_disk))
        .routes(routes!(rename_disk))
        .routes(routes!(smart_disk))
        .routes(routes!(eject_disk))
        .routes(routes!(locks, set_lock))
}
