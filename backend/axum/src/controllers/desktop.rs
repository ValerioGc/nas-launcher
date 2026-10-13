use crate::{
    http::{error::ApiFailure, extract::ValidJson, state::AppState},
    services::desktop::{self, FolderPath, FolderShortcut, Order, Shortcut, ShortcutKind},
};
use axum::{
    extract::{Path, State},
    Json,
};
use sqlx::SqlitePool;
use utoipa_axum::{router::OpenApiRouter, routes};

/// The desktop icons in order
#[utoipa::path(get, path = "/desktop/shortcuts", responses((status = OK, body = Vec<Shortcut>)))]
async fn shortcuts(State(db): State<SqlitePool>) -> Result<Json<Vec<Shortcut>>, ApiFailure> {
    desktop::list(&db).await.map(Json)
}

/// Adds the icon of an application and answers with the list
#[utoipa::path(
    post,
    path = "/desktop/shortcuts/{id}",
    responses((status = OK, body = Vec<Shortcut>))
)]
async fn add_shortcut(
    State(db): State<SqlitePool>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Shortcut>>, ApiFailure> {
    desktop::add_app(&db, &id).await.map(Json)
}

/// Removes the icon of an application and answers with the list
#[utoipa::path(
    delete,
    path = "/desktop/shortcuts/{id}",
    responses((status = OK, body = Vec<Shortcut>))
)]
async fn remove_shortcut(
    State(db): State<SqlitePool>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Shortcut>>, ApiFailure> {
    desktop::remove(&db, ShortcutKind::App, &id).await.map(Json)
}

/// Saves a folder shortcut from nas-file-manager.
#[utoipa::path(post, path = "/desktop/folders", request_body = FolderShortcut,
    responses((status = OK, body = Vec<Shortcut>)))]
async fn add_folder(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<FolderShortcut>,
) -> Result<Json<Vec<Shortcut>>, ApiFailure> {
    desktop::add_folder(&db, input).await.map(Json)
}

/// Removes only the desktop shortcut, leaving the folder untouched.
#[utoipa::path(delete, path = "/desktop/folders", request_body = FolderPath,
    responses((status = OK, body = Vec<Shortcut>)))]
async fn remove_folder(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<FolderPath>,
) -> Result<Json<Vec<Shortcut>>, ApiFailure> {
    desktop::remove_folder(&db, &input.path).await.map(Json)
}

/// Puts the listed icons first in that order and the others after
#[utoipa::path(
    put,
    path = "/desktop/shortcuts/order",
    request_body = Order,
    responses((status = OK, body = Vec<Shortcut>))
)]
async fn reorder(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<Order>,
) -> Result<Json<Vec<Shortcut>>, ApiFailure> {
    desktop::reorder(&db, input).await.map(Json)
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(shortcuts))
        .routes(routes!(reorder))
        .routes(routes!(add_shortcut, remove_shortcut))
        .routes(routes!(add_folder, remove_folder))
}
