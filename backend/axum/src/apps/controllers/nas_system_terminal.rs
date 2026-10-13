use crate::{
    apps::services::nas_system_terminal::{self, TerminalEvent, TerminalSettings},
    http::{error::ApiFailure, extract::ValidJson, state::AppState},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures_util::{Stream, StreamExt};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::convert::Infallible;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

/// A shell for the window as server-sent events named session and done around the output lines
#[utoipa::path(
    get,
    path = "/terminal/session",
    responses((status = OK, content_type = "text/event-stream", body = String))
)]
async fn open_session() -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiFailure> {
    let events = nas_system_terminal::open().await?.map(|event| {
        Ok(match event {
            TerminalEvent::Session(id) => Event::default().event("session").data(id),
            TerminalEvent::Line(line) => Event::default().data(line),
            TerminalEvent::Done { code, cwd } => {
                Event::default().event("done").data(format!("{code} {cwd}"))
            }
        })
    });
    Ok(Sse::new(events).keep_alive(KeepAlive::default()))
}

/// The system journal as server-sent events with one line each
#[utoipa::path(
    get,
    path = "/terminal/log",
    responses((status = OK, content_type = "text/event-stream", body = String))
)]
async fn follow_log() -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiFailure> {
    let lines = nas_system_terminal::journal()?;
    Ok(
        Sse::new(lines.map(|line| Ok(Event::default().data(line))))
            .keep_alive(KeepAlive::default()),
    )
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct Command {
    command: String,
}

/// Runs one line in the shell of a session
#[utoipa::path(
    post,
    path = "/terminal/session/{id}/input",
    request_body = Command,
    responses((status = NO_CONTENT))
)]
async fn run(
    Path(id): Path<String>,
    ValidJson(input): ValidJson<Command>,
) -> Result<StatusCode, ApiFailure> {
    nas_system_terminal::run(&id, &input.command).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// How the terminal looks
#[utoipa::path(get, path = "/settings/terminal", responses((status = OK, body = TerminalSettings)))]
async fn settings(State(db): State<SqlitePool>) -> Result<Json<TerminalSettings>, ApiFailure> {
    nas_system_terminal::load_settings(&db).await.map(Json)
}

/// Saves how the terminal looks
#[utoipa::path(
    put,
    path = "/settings/terminal",
    request_body = TerminalSettings,
    responses((status = OK, body = TerminalSettings))
)]
async fn save_settings(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<TerminalSettings>,
) -> Result<Json<TerminalSettings>, ApiFailure> {
    nas_system_terminal::save_settings(&db, input)
        .await
        .map(Json)
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(follow_log))
        .routes(routes!(open_session))
        .routes(routes!(run))
        .routes(routes!(settings, save_settings))
}
