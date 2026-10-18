use axum::{extract::DefaultBodyLimit, middleware, Router};
use utoipa_axum::router::OpenApiRouter;

use crate::{
    apps, controllers,
    controllers::health,
    http::{
        app_channel::identify,
        boundary::{api_boundary, no_index},
        guard::access_guard,
        openapi,
        proxy::app_proxy,
        state::AppState,
    },
    widgets,
};

fn api_routes() -> OpenApiRouter<AppState> {
    health::health_routes().nest(
        "/api",
        controllers::routes()
            .merge(apps::routes())
            .merge(controllers::terminal::routes())
            .merge(widgets::routes()),
    )
}

/// The OpenAPI document of every route of the launcher.
pub fn openapi() -> utoipa::openapi::OpenApi {
    openapi::document(api_routes().into_openapi())
}

pub fn build_router(state: AppState) -> Router {
    let (routes, _) = api_routes().split_for_parts();
    routes
        .merge(controllers::mail::internal_routes())
        .fallback(health::not_found)
        .layer(middleware::from_fn(api_boundary))
        .layer(DefaultBodyLimit::max(32 * 1024))
        .layer(middleware::from_fn(app_proxy))
        .layer(middleware::from_fn(no_index))
        .layer(middleware::from_fn_with_state(state.clone(), access_guard))
        .with_state(state)
}

/// The routes the processes of the applications reach on their own socket, and nothing else.
pub fn build_app_router(state: AppState) -> Router {
    controllers::app_channel::routes()
        .layer(middleware::from_fn(api_boundary))
        .layer(DefaultBodyLimit::max(32 * 1024))
        .layer(middleware::from_fn(identify))
        .with_state(state)
}
