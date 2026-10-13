//! Task routes; operation state belongs to [`crate::widgets::services::tasks`].

use crate::{
    http::{error::ApiFailure, extract::ValidJson, reply::Removed, state::AppState},
    widgets::services::tasks::{self, NewTask, Task, TaskUpdate},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sqlx::SqlitePool;
use utoipa_axum::{router::OpenApiRouter, routes};

/// The register of ongoing operations
#[utoipa::path(get, path = "/tasks", responses((status = OK, body = Vec<Task>)))]
pub(crate) async fn list(State(db): State<SqlitePool>) -> Result<Json<Vec<Task>>, ApiFailure> {
    tasks::list(&db).await.map(Json)
}

/// Registers an operation
#[utoipa::path(post, path = "/tasks", request_body = NewTask, responses((status = OK, body = Task)))]
async fn register(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<NewTask>,
) -> Result<Json<Task>, ApiFailure> {
    tasks::register(&db, input).await.map(Json)
}

/// What the owner reports while it works
#[utoipa::path(
    patch,
    path = "/tasks/{id}",
    request_body = TaskUpdate,
    responses((status = OK, body = Task))
)]
async fn update(
    State(db): State<SqlitePool>,
    Path(id): Path<String>,
    ValidJson(input): ValidJson<TaskUpdate>,
) -> Result<Json<Task>, ApiFailure> {
    tasks::update(&db, &id, input).await.map(Json)
}

/// Asks the owner to pause resume or stop an operation
#[utoipa::path(
    post,
    path = "/tasks/{id}/{action}",
    params(
        ("id" = String, Path),
        ("action" = String, Path, description = "pause resume or stop")
    ),
    responses((status = OK, body = Task))
)]
async fn request(
    State(db): State<SqlitePool>,
    Path((id, action)): Path<(String, String)>,
) -> Result<Json<Task>, ApiFailure> {
    tasks::request(&db, &id, &action).await.map(Json)
}

/// Removes an operation
#[utoipa::path(delete, path = "/tasks/{id}", responses((status = NO_CONTENT)))]
async fn remove(
    State(db): State<SqlitePool>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiFailure> {
    tasks::remove(&db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Clears every finished operation
#[utoipa::path(post, path = "/tasks/finished", responses((status = OK, body = Removed)))]
async fn clear_finished(State(db): State<SqlitePool>) -> Result<Json<Removed>, ApiFailure> {
    let removed = tasks::clear_finished(&db).await?;
    Ok(Json(Removed { removed }))
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, register))
        .routes(routes!(clear_finished))
        .routes(routes!(update, remove))
        .routes(routes!(request))
}
