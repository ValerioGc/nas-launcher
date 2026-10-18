use crate::{
    http::{error::ApiFailure, state::AppState},
    services::manifest::{self, Provider},
};
use axum::{extract::Path, Json};
use utoipa_axum::{router::OpenApiRouter, routes};

/// The installed applications that provide a capability and where the browser reaches them
#[utoipa::path(
    get,
    path = "/capabilities/{capability}",
    responses((status = OK, body = Vec<Provider>))
)]
async fn providers(Path(capability): Path<String>) -> Result<Json<Vec<Provider>>, ApiFailure> {
    if !manifest::is_identifier(&capability) {
        return Err(ApiFailure::bad("invalid capability"));
    }
    Ok(Json(manifest::providers(
        &manifest::all().await,
        &capability,
    )))
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(providers))
}
