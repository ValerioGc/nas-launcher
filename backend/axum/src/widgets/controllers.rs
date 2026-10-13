pub mod settings;
pub mod tasks;

use crate::http::state::AppState;
use utoipa_axum::router::OpenApiRouter;

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .merge(settings::routes())
        .merge(tasks::routes())
}
