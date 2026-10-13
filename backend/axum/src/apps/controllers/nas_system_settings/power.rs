use crate::{
    apps::services::nas_system_settings::power::{self, Busy, PowerAction, PowerOutcome},
    http::{error::ApiFailure, state::AppState},
    utils::{self, Mode},
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use sqlx::SqlitePool;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct PowerMode {
    simulated: bool,
}

/// Whether restart and shutdown are simulated
#[utoipa::path(get, path = "/system/power", responses((status = OK, body = PowerMode)))]
async fn mode() -> Json<PowerMode> {
    Json(PowerMode {
        simulated: !utils::is_real(Mode::Power),
    })
}

/// What a restart or a shutdown would interrupt (`tasks`) or has to wait for (`jobs`)
#[utoipa::path(get, path = "/system/power/busy", responses((status = OK, body = Busy)))]
async fn busy(State(db): State<SqlitePool>) -> Json<Busy> {
    Json(power::busy(&db).await)
}

/// Restarts or shuts down the machine, unless a job of root is running
#[utoipa::path(
    post,
    path = "/system/actions/{action}",
    params(("action" = String, Path, description = "restart or shutdown")),
    responses(
        (status = OK, body = PowerOutcome),
        (status = CONFLICT, description = "a job of root is running: the NAS waits for it")
    )
)]
async fn action(
    State(db): State<SqlitePool>,
    Path(action): Path<String>,
) -> Result<Json<PowerOutcome>, ApiFailure> {
    let action =
        PowerAction::parse(&action).ok_or_else(|| ApiFailure::bad("invalid system action"))?;
    power::run(&db, action).await.map(Json)
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(action))
        .routes(routes!(mode))
        .routes(routes!(busy))
}
