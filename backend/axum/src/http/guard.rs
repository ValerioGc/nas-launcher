//! Enforces saved access rules for direct connections; system firewall rules live in
//! [`crate::apps::services::nas_system_settings::access`]. Requests without peer information bypass
//! this guard.

use crate::{
    apps::services::nas_system_settings::access,
    http::{error::ApiFailure, extract::peer_address},
};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use sqlx::SqlitePool;

/// Enforces access rules for incoming requests based on their peer address.
/// Requests without peer information bypass this guard.
///
/// **Parameters**
/// - `State(db)`: The shared database connection pool.
/// - `request`: The incoming HTTP request.
/// - `next`: The next middleware or handler in the chain.
///
/// **Returns**
///  - A `Response` indicating whether the request is allowed or forbidden.
///
pub async fn access_guard(State(db): State<SqlitePool>, request: Request, next: Next) -> Response {
    let allowed = match peer_address(request.extensions()) {
        None => true,
        Some(address) => access::allows(&db, address).await,
    };

    if allowed {
        return next.run(request).await;
    }

    ApiFailure(
        StatusCode::FORBIDDEN,
        "this address is not allowed to reach the NAS".into(),
    )
    .into_response()
}
