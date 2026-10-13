//! State and routes for header widgets.

pub mod controllers;
pub mod services;

use crate::http::state::AppState;
use utoipa_axum::router::OpenApiRouter;

pub fn routes() -> OpenApiRouter<AppState> {
    controllers::routes()
}
