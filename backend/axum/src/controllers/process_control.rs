use crate::{
    http::{error::ApiFailure, extract::ValidJson, state::AppState},
    services::{manifest, process_control as actions},
};
use axum::{
    extract::{Path, Request},
    http::Uri,
    response::Response,
    Json,
};
use utoipa_axum::{router::OpenApiRouter, routes};

/// Compatibility forwarding for older System Monitor clients. The application owns the readings.
#[utoipa::path(
    get,
    path = "/system/telemetry",
    responses((status = OK, body = serde_json::Value))
)]
pub(crate) async fn telemetry(request: Request) -> Result<Response, ApiFailure> {
    let app = manifest::read("nas-system-monitor")
        .await
        .ok_or_else(|| ApiFailure::missing("System Monitor is not installed"))?;
    Ok(forward_telemetry(app, request).await)
}

async fn forward_telemetry(app: manifest::Manifest, mut request: Request) -> Response {
    *request.uri_mut() = Uri::from_static("/api/telemetry");
    crate::http::proxy::forward(app, request).await
}

/// Terminates a process, or restarts the service it runs in
#[utoipa::path(
    post,
    path = "/system/processes/{id}",
    params(("id" = String, Path, description = "the id of the process")),
    request_body = actions::ProcessRequest,
    responses((status = OK, body = actions::ProcessResult))
)]
pub(crate) async fn act(
    Path(id): Path<String>,
    ValidJson(input): ValidJson<actions::ProcessRequest>,
) -> Result<Json<actions::ProcessResult>, ApiFailure> {
    let id = id
        .parse()
        .map_err(|_| ApiFailure::bad(format!("not a process id: {id}")))?;
    Ok(Json(actions::act(id, input).await?))
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(telemetry))
        .routes(routes!(act))
}

#[cfg(test)]
#[path = "../../tests/controllers/process_control.rs"]
mod tests;
