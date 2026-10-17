use crate::{
    apps::services::nas_system_settings::network::{
        self, AdapterRequest, NetworkState, WifiRequest,
    },
    http::error::ApiFailure,
    http::extract::ValidJson,
    http::state::AppState,
};
use axum::{extract::State, Json};
use sqlx::SqlitePool;
use utoipa_axum::{router::OpenApiRouter, routes};

/// The current connection and the networks seen
#[utoipa::path(get, path = "/network", responses((status = OK, body = NetworkState)))]
pub(crate) async fn status(State(db): State<SqlitePool>) -> Result<Json<NetworkState>, ApiFailure> {
    network::status(&db, false).await.map(Json)
}

/// Looks for networks again and answers with the state
#[utoipa::path(post, path = "/network/scan", responses((status = OK, body = NetworkState)))]
async fn scan(State(db): State<SqlitePool>) -> Result<Json<NetworkState>, ApiFailure> {
    network::status(&db, true).await.map(Json)
}

/// Joins a wireless network from the list
#[utoipa::path(
    post,
    path = "/network/wifi",
    request_body = WifiRequest,
    responses((status = OK, body = NetworkState))
)]
async fn connect_wifi(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<WifiRequest>,
) -> Result<Json<NetworkState>, ApiFailure> {
    network::connect_wifi(&db, input).await.map(Json)
}

/// Goes back to the cable
#[utoipa::path(post, path = "/network/wired", responses((status = OK, body = NetworkState)))]
async fn connect_wired(State(db): State<SqlitePool>) -> Result<Json<NetworkState>, ApiFailure> {
    network::connect_wired(&db).await.map(Json)
}

/// Leaves the wireless network when that is not the way in
#[utoipa::path(post, path = "/network/disconnect", responses((status = OK, body = NetworkState)))]
async fn disconnect(State(db): State<SqlitePool>) -> Result<Json<NetworkState>, ApiFailure> {
    network::disconnect_wifi(&db).await.map(Json)
}

/// Turns a network card off, or back on; never the one the NAS is reached through
#[utoipa::path(
    post,
    path = "/network/adapter",
    request_body = AdapterRequest,
    responses((status = OK, body = NetworkState))
)]
async fn set_adapter(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<AdapterRequest>,
) -> Result<Json<NetworkState>, ApiFailure> {
    network::set_adapter(&db, input).await.map(Json)
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(status))
        .routes(routes!(scan))
        .routes(routes!(connect_wifi))
        .routes(routes!(connect_wired))
        .routes(routes!(disconnect))
        .routes(routes!(set_adapter))
}
