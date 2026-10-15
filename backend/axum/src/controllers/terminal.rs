use crate::{
    http::{error::ApiFailure, extract::ValidJson, state::AppState},
    services::terminal::{self, TerminalSettings},
};
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures_util::{Stream, StreamExt};
use sqlx::SqlitePool;
use std::convert::Infallible;
use utoipa_axum::{router::OpenApiRouter, routes};

/// The system journal as server-sent events with one line each
#[utoipa::path(
    get,
    path = "/terminal/log",
    responses((status = OK, content_type = "text/event-stream", body = String))
)]
async fn follow_log() -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiFailure> {
    let lines = terminal::journal()?;
    Ok(
        Sse::new(lines.map(|line| Ok(Event::default().data(line))))
            .keep_alive(KeepAlive::default()),
    )
}

/// How the terminal looks
#[utoipa::path(get, path = "/settings/terminal", responses((status = OK, body = TerminalSettings)))]
async fn settings(State(db): State<SqlitePool>) -> Result<Json<TerminalSettings>, ApiFailure> {
    terminal::load_settings(&db).await.map(Json)
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
    terminal::save_settings(&db, input).await.map(Json)
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(follow_log))
        .routes(routes!(settings, save_settings))
}
