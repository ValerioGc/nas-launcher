use crate::{
    apps::services::nas_system_settings::config::{self, Config},
    http::{
        error::ApiFailure,
        extract::{Reauthenticated, ValidJson},
        state::AppState,
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
struct ExportQuery {
    #[serde(default)]
    apps: String,
}

/// The applications whose settings can go in the file
#[utoipa::path(
    get,
    path = "/system/config/apps",
    responses((status = OK, body = Vec<String>))
)]
async fn exportable() -> Json<Vec<&'static str>> {
    Json(config::EXPORTABLE.to_vec())
}

/// The configuration of the NAS, for the page to encrypt and save. The gateway checks the password
/// first.
#[utoipa::path(
    get,
    path = "/system/config",
    params(ExportQuery),
    responses(
        (status = OK, body = Config),
        (status = FORBIDDEN, description = "the password was not confirmed by the gateway")
    )
)]
async fn export(
    _: Reauthenticated,
    State(db): State<SqlitePool>,
    Query(query): Query<ExportQuery>,
) -> Result<Json<Config>, ApiFailure> {
    let apps: Vec<String> = query
        .apps
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .collect();
    config::export(&db, &apps).await.map(Json)
}

/// Replaces the configuration with the one of a file the page decrypted. The gateway checks the
/// password first.
#[utoipa::path(
    post,
    path = "/system/config",
    request_body = Config,
    responses(
        (status = OK, body = Config),
        (status = FORBIDDEN, description = "the password was not confirmed by the gateway")
    )
)]
async fn import(
    _: Reauthenticated,
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<Config>,
) -> Result<Json<Config>, ApiFailure> {
    config::import(&db, input).await.map(Json)
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(export, import))
        .routes(routes!(exportable))
}
