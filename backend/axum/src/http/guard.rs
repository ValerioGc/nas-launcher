//! Enforces saved access rules for direct connections; system firewall rules live in
//! [`crate::apps::services::nas_system_settings::access`]. Requests without peer information bypass
//! this guard.

use crate::{
    apps::services::nas_system_settings::access::AccessRules,
    http::{error::ApiFailure, extract::peer_address},
};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use sqlx::SqlitePool;

pub async fn access_guard(
    State(db): State<SqlitePool>,
    State(rules): State<AccessRules>,
    request: Request,
    next: Next,
) -> Response {
    let allowed = match peer_address(request.extensions()) {
        None => true,
        Some(address) => rules.allows(&db, address).await,
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

#[cfg(test)]
#[path = "../../tests/http/guard.rs"]
mod tests;
