use crate::{
    apps::services::nas_notification_manager::{self, NotificationFilter, NotificationSettings},
    http::error::ApiFailure,
    http::extract::ValidJson,
    http::reply::Removed,
    http::state::AppState,
    services::notifications::{self, NewNotification, Notification},
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

/// The notifications that match the filter
#[utoipa::path(
    get,
    path = "/notifications",
    params(NotificationFilter),
    responses((status = OK, body = Vec<Notification>))
)]
pub(crate) async fn list(
    State(db): State<SqlitePool>,
    Query(filter): Query<NotificationFilter>,
) -> Result<Json<Vec<Notification>>, ApiFailure> {
    filter.validate()?;
    nas_notification_manager::list(&db, &filter).await.map(Json)
}

/// Records a notification
#[utoipa::path(
    post,
    path = "/notifications",
    request_body = NewNotification,
    responses((status = OK, body = Notification))
)]
async fn create(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<NewNotification>,
) -> Result<Json<Notification>, ApiFailure> {
    notifications::create(&db, input).await.map(Json)
}

/// The sources present in the log
#[utoipa::path(get, path = "/notifications/sources", responses((status = OK, body = Vec<String>)))]
async fn sources(State(db): State<SqlitePool>) -> Result<Json<Vec<String>>, ApiFailure> {
    nas_notification_manager::sources(&db).await.map(Json)
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct Read {
    read: bool,
}

/// Marks a notification read or unread
#[utoipa::path(
    patch,
    path = "/notifications/{id}",
    request_body = Read,
    responses((status = OK, body = Notification))
)]
async fn mark_read(
    State(db): State<SqlitePool>,
    Path(id): Path<String>,
    ValidJson(input): ValidJson<Read>,
) -> Result<Json<Notification>, ApiFailure> {
    nas_notification_manager::mark_read(&db, &id, input.read)
        .await
        .map(Json)
}

/// The log behind a notification
#[utoipa::path(
    get,
    path = "/notifications/{id}/log",
    responses((status = OK, body = nas_notification_manager::log::NotificationLog))
)]
pub(crate) async fn log(
    State(db): State<SqlitePool>,
    Path(id): Path<String>,
) -> Result<Json<nas_notification_manager::log::NotificationLog>, ApiFailure> {
    let notification = nas_notification_manager::get(&db, &id).await?;
    nas_notification_manager::log::read(&notification)
        .await
        .map(Json)
}

/// Deletes a notification
#[utoipa::path(delete, path = "/notifications/{id}", responses((status = NO_CONTENT)))]
async fn remove(
    State(db): State<SqlitePool>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiFailure> {
    nas_notification_manager::remove(&db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The delivery preferences
#[utoipa::path(
    get,
    path = "/settings/notifications",
    responses((status = OK, body = NotificationSettings))
)]
async fn settings(State(db): State<SqlitePool>) -> Result<Json<NotificationSettings>, ApiFailure> {
    nas_notification_manager::load_settings(&db).await.map(Json)
}

/// Saves the delivery preferences
#[utoipa::path(
    put,
    path = "/settings/notifications",
    request_body = NotificationSettings,
    responses((status = OK, body = NotificationSettings))
)]
async fn save_settings(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<NotificationSettings>,
) -> Result<Json<NotificationSettings>, ApiFailure> {
    nas_notification_manager::save_settings(&db, input)
        .await
        .map(Json)
}

#[derive(Serialize, ToSchema)]
struct Changed {
    changed: u64,
}

/// Marks every unread notification read
#[utoipa::path(post, path = "/notifications/read", responses((status = OK, body = Changed)))]
async fn mark_all_read(State(db): State<SqlitePool>) -> Result<Json<Changed>, ApiFailure> {
    let changed = nas_notification_manager::mark_all_read(&db).await?;
    Ok(Json(Changed { changed }))
}

/// Deletes the notifications already read
#[utoipa::path(post, path = "/notifications/read/clear", responses((status = OK, body = Removed)))]
async fn remove_read(State(db): State<SqlitePool>) -> Result<Json<Removed>, ApiFailure> {
    let removed = nas_notification_manager::remove_read(&db).await?;
    Ok(Json(Removed { removed }))
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(sources))
        .routes(routes!(mark_read, remove))
        .routes(routes!(log))
        .routes(routes!(mark_all_read))
        .routes(routes!(remove_read))
        .routes(routes!(settings, save_settings))
}
