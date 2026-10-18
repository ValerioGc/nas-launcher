use crate::{
    apps::services::nas_system_settings::reset::{self, ResetState},
    http::{error::ApiFailure, extract::Reauthenticated, state::AppState},
};
use axum::{http::StatusCode, Json};
use utoipa_axum::{router::OpenApiRouter, routes};

/// Whether the NAS can be reset here
#[utoipa::path(get, path = "/system/reset", responses((status = OK, body = ResetState)))]
async fn state() -> Json<ResetState> {
    Json(reset::state().await)
}

/// Resets the NAS and starts the wizard again. The gateway checks the password first.
#[utoipa::path(
    post,
    path = "/system/reset",
    responses(
        (status = ACCEPTED, body = ResetState),
        (status = FORBIDDEN, description = "the password was not confirmed by the gateway"),
        (status = SERVICE_UNAVAILABLE, description = "the reset is not possible here")
    )
)]
async fn start(_: Reauthenticated) -> Result<(StatusCode, Json<ResetState>), ApiFailure> {
    reset::start()
        .await
        .map(|state| (StatusCode::ACCEPTED, Json(state)))
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(state, start))
}
