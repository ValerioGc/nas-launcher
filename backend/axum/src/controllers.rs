pub mod app_channel;
pub mod caches;
pub mod capabilities;
pub mod desktop;
pub mod health;
pub mod mail;
pub mod process_control;
pub mod setup;
pub mod storage;
pub mod terminal;

use crate::http::state::AppState;
use utoipa_axum::router::OpenApiRouter;

/// Launcher routes, mounted under `/api`.
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .merge(caches::routes())
        .merge(capabilities::routes())
        .merge(health::routes())
        .merge(mail::routes())
        .merge(process_control::routes())
        .merge(desktop::routes())
        .merge(setup::routes())
        .merge(storage::routes())
}
