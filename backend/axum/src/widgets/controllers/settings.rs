use crate::{
    http::{error::ApiFailure, extract::ValidJson, state::AppState},
    widgets::services::settings::{self, WidgetSettings},
};
use axum::{extract::State, Json};
use sqlx::SqlitePool;
use utoipa_axum::{router::OpenApiRouter, routes};

#[utoipa::path(get, path = "/settings/widgets", responses((status = OK, body = WidgetSettings)))]
async fn read(State(db): State<SqlitePool>) -> Result<Json<WidgetSettings>, ApiFailure> {
    settings::load(&db).await.map(Json)
}
#[utoipa::path(
    put,
    path = "/settings/widgets",
    request_body = WidgetSettings,
    responses((status = OK, body = WidgetSettings))
)]
async fn save_settings(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<WidgetSettings>,
) -> Result<Json<WidgetSettings>, ApiFailure> {
    settings::save(&db, input).await.map(Json)
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(read, save_settings))
}
