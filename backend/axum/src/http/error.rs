use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

/// Body shared by every failed response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, utoipa::ToSchema)]
pub struct ApiError {
    pub error: String,
}

/// Typed failure of a handler or of a service.
#[derive(Debug)]
pub struct ApiFailure(pub StatusCode, pub String);
impl ApiFailure {
    pub fn bad(message: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, message.into())
    }
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self(StatusCode::FORBIDDEN, message.into())
    }
    pub fn missing(resource: impl Into<String>) -> Self {
        Self(StatusCode::NOT_FOUND, resource.into())
    }

    pub fn unavailable(resource: impl Into<String>) -> Self {
        Self(StatusCode::SERVICE_UNAVAILABLE, resource.into())
    }

    pub fn unsupported(what: impl Into<String>) -> Self {
        Self(StatusCode::NOT_IMPLEMENTED, what.into())
    }
}
impl IntoResponse for ApiFailure {
    fn into_response(self) -> axum::response::Response {
        (self.0, Json(ApiError { error: self.1 })).into_response()
    }
}
