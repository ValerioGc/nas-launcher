//! Apps compiled into the launcher.

pub mod controllers;
pub mod services;

use crate::http::state::AppState;
use services::nas_system_settings::{system_updates::Job, updates::Component};
use std::collections::BTreeMap;
use utoipa_axum::router::OpenApiRouter;

#[derive(Default)]
pub struct AppsState {
    pub apt_job: Option<Job>,
    pub available_updates: BTreeMap<Component, String>,
}

pub fn routes() -> OpenApiRouter<AppState> {
    controllers::routes()
}
