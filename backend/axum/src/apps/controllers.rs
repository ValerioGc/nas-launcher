pub mod antivirus;
pub mod nas_app_manager;
pub mod nas_notification_manager;
pub mod nas_system_settings;

use crate::http::state::AppState;
use utoipa_axum::router::OpenApiRouter;

/// Routes of every app, mounted under `/api/{app-id}` next to the launcher ones.
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .merge(antivirus::routes())
        .merge(nas_app_manager::routes())
        .merge(nas_system_settings::routes())
        .merge(nas_notification_manager::routes())
}
