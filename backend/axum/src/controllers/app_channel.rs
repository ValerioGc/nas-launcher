//! Authenticated routes for application processes on [`crate::http::app_channel`].
//! Reads expose the same state as the public API. Notification and operation writes
//! are restricted to the authenticated application's identity and owned records.

use crate::{
    apps::controllers::{nas_app_manager, nas_notification_manager, nas_system_settings::network},
    controllers::{health, process_control},
    http::{app_channel::Caller, error::ApiFailure, extract::ValidJson, state::AppState},
    services::{
        background, caches,
        cleanup::{self, Request as CleanupRequest},
        manifest,
        notifications::{self, NewNotification, Notification},
        shares::{self, Shares, Sharing},
        sync_ports,
    },
    widgets::{
        controllers::tasks as task_routes,
        services::tasks::{self, NewTask, Task, TaskUpdate},
    },
};
use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post, put},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

fn speaks_for(caller: &Caller, source: &str) -> Result<(), ApiFailure> {
    if caller.0 == source {
        Ok(())
    } else {
        Err(ApiFailure::forbidden(format!(
            "{} may only write as itself, not as {source}",
            caller.0
        )))
    }
}

/// The operation, if it belongs to the application asking.
async fn owned(db: &SqlitePool, caller: &Caller, id: &str) -> Result<Task, ApiFailure> {
    let task = tasks::get(db, id).await?;
    speaks_for(caller, &task.source)?;
    Ok(task)
}

async fn notify(
    State(db): State<SqlitePool>,
    Extension(caller): Extension<Caller>,
    ValidJson(input): ValidJson<NewNotification>,
) -> Result<Json<Notification>, ApiFailure> {
    speaks_for(&caller, &input.source)?;
    notifications::create(&db, input).await.map(Json)
}

/// Registers an operation or replaces one owned by the requesting application.
async fn register(
    State(db): State<SqlitePool>,
    Extension(caller): Extension<Caller>,
    ValidJson(input): ValidJson<NewTask>,
) -> Result<Json<Task>, ApiFailure> {
    speaks_for(&caller, &input.source)?;
    match tasks::get(&db, &input.id).await {
        Ok(existing) => speaks_for(&caller, &existing.source)?,
        Err(ApiFailure(StatusCode::NOT_FOUND, _)) => {}
        Err(failure) => return Err(failure),
    }
    tasks::register(&db, input).await.map(Json)
}

async fn update(
    State(db): State<SqlitePool>,
    Extension(caller): Extension<Caller>,
    Path(id): Path<String>,
    ValidJson(input): ValidJson<TaskUpdate>,
) -> Result<Json<Task>, ApiFailure> {
    owned(&db, &caller, &id).await?;
    tasks::update(&db, &id, input).await.map(Json)
}

async fn remove(
    State(db): State<SqlitePool>,
    Extension(caller): Extension<Caller>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiFailure> {
    owned(&db, &caller, &id).await?;
    tasks::remove(&db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Whether the application has work to do without its window.
#[derive(Debug, Deserialize)]
struct Background {
    needed: bool,
}

/// Updates background-work state for the authenticated application.
async fn declare_background(
    State(db): State<SqlitePool>,
    Extension(caller): Extension<Caller>,
    ValidJson(input): ValidJson<Background>,
) -> Result<StatusCode, ApiFailure> {
    background::declare(&db, &caller.0, input.needed).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Per-user inotify watch limit used for watched folders.
const INOTIFY_WATCHES: &str = "/proc/sys/fs/inotify/max_user_watches";

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Inotify {
    max_user_watches: u64,
}

/// Reports the inotify limit to sandboxed applications without exposing `/proc/sys`.
async fn inotify() -> Result<Json<Inotify>, ApiFailure> {
    let text = tokio::fs::read_to_string(INOTIFY_WATCHES)
        .await
        .map_err(|_| ApiFailure::unavailable("the limit of inotify"))?;
    let max_user_watches = text
        .trim()
        .parse()
        .map_err(|_| ApiFailure::unavailable("the limit of inotify"))?;
    Ok(Json(Inotify { max_user_watches }))
}

/// Reports active sync state and reconciles local-network Syncthing ports.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sync {
    active: bool,
}

async fn declare_sync(
    Extension(caller): Extension<Caller>,
    ValidJson(input): ValidJson<Sync>,
) -> Result<StatusCode, ApiFailure> {
    sync_ports::declare(&caller.0, input.active).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Only an application whose manifest asks for `permissions.cleanup` cleans the system.
async fn may_clean(caller: &Caller) -> Result<(), ApiFailure> {
    match manifest::read(&caller.0).await {
        Some(app) if app.cleanup => Ok(()),
        _ => Err(ApiFailure::forbidden(format!(
            "{} does not ask for permissions.cleanup",
            caller.0
        ))),
    }
}

/// An overview, a preview or a cleanup of one item of the fixed list (`services::cleanup`).
async fn clean(
    Extension(caller): Extension<Caller>,
    ValidJson(input): ValidJson<CleanupRequest>,
) -> Result<Json<serde_json::Value>, ApiFailure> {
    may_clean(&caller).await?;
    cleanup::run(input).await.map(Json)
}

/// The caches of the NAS, for the application that cleans.
async fn list_caches(
    Extension(caller): Extension<Caller>,
) -> Result<Json<Vec<caches::Cache>>, ApiFailure> {
    may_clean(&caller).await?;
    Ok(Json(caches::list().await))
}

async fn clear_cache(
    State(db): State<SqlitePool>,
    Extension(caller): Extension<Caller>,
    Path((app, id)): Path<(String, String)>,
) -> Result<StatusCode, ApiFailure> {
    may_clean(&caller).await?;
    caches::clear(&db, &app, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Whether the application can share folders, and who signs in to a protected one.
async fn sharing() -> Json<Sharing> {
    Json(shares::sharing().await)
}

/// Replaces the complete share list owned by the authenticated application.
async fn declare_shares(
    Extension(caller): Extension<Caller>,
    ValidJson(input): ValidJson<Shares>,
) -> Result<StatusCode, ApiFailure> {
    shares::declare(&caller.0, &input.shares).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn terminal_only(
    Extension(caller): Extension<Caller>,
    request: Request,
    next: Next,
) -> Response {
    if caller.0 != "nas-system-terminal" {
        return ApiFailure::forbidden(
            "only the terminal application may use its journal and settings",
        )
        .into_response();
    }
    next.run(request).await
}

pub fn routes() -> Router<AppState> {
    let terminal = crate::controllers::terminal::routes()
        .split_for_parts()
        .0
        .route_layer(middleware::from_fn(terminal_only));
    Router::new()
        .nest("/api", terminal)
        .route("/api/apps", get(nas_app_manager::installed))
        .route("/api/system/telemetry", get(process_control::telemetry))
        .route("/api/system/inotify", get(inotify))
        .route("/api/network", get(network::status))
        .route(
            "/api/notifications",
            get(nas_notification_manager::list).post(notify),
        )
        .route(
            "/api/notifications/{id}/log",
            get(nas_notification_manager::log),
        )
        .route("/api/tasks", get(task_routes::list).post(register))
        .route("/api/tasks/{id}", patch(update).delete(remove))
        .route("/api/apps/self/background", put(declare_background))
        .route("/api/apps/self/shares", get(sharing).put(declare_shares))
        .route("/api/apps/self/cleanup", post(clean))
        .route("/api/apps/self/sync", put(declare_sync))
        .route("/api/caches", get(list_caches))
        .route("/api/caches/{app}/{id}", delete(clear_cache))
        .fallback(health::not_found)
}

#[cfg(test)]
#[path = "../../tests/controllers/app_channel.rs"]
mod tests;
