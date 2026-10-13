use crate::{
    http::{error::ApiFailure, extract::ValidJson, state::AppState},
    services::setup::{self, FirstConfiguration, SetupState},
};
use axum::{extract::State, Json};
use sqlx::SqlitePool;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Whether the wizard was completed and the steps after it
#[utoipa::path(get, path = "/setup", responses((status = OK, body = SetupState)))]
async fn state(State(db): State<SqlitePool>) -> Result<Json<SetupState>, ApiFailure> {
    setup::state(&db).await.map(Json)
}

/// Saves what the wizard hands to the launcher, once
#[utoipa::path(
    post,
    path = "/setup/complete",
    request_body = FirstConfiguration,
    responses((status = OK, body = SetupState))
)]
async fn complete(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<FirstConfiguration>,
) -> Result<Json<SetupState>, ApiFailure> {
    setup::complete(&db, input).await.map(Json)
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(state))
        .routes(routes!(complete))
}
