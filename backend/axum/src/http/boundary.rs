use crate::http::error::ApiFailure;
use axum::{
    extract::Request,
    http::{header, HeaderName, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Allowed content types for mutation requests.
const MUTATION_TYPES: [&str; 5] = [
    "application/json",
    "image/svg+xml",
    "image/png",
    "image/jpeg",
    "image/webp",
];

/// Enforces content type and same-origin requirements for mutation requests.
pub async fn api_boundary(request: Request, next: Next) -> Response {
    let mutation = !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    if mutation
        && !request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .is_some_and(|v| MUTATION_TYPES.contains(&v.trim()))
    {
        return ApiFailure(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "application/json required".into(),
        )
        .into_response();
    }

    if mutation
        && request
            .headers()
            .get("sec-fetch-site")
            .is_some_and(|v| v != "same-origin" && v != "none")
    {
        return ApiFailure(
            StatusCode::FORBIDDEN,
            "cross-origin mutation rejected".into(),
        )
        .into_response();
    }

    let response = next.run(request).await;
    if response.status().is_client_error()
        && !response
            .headers()
            .get(header::CONTENT_TYPE)
            .is_some_and(|v| v == "application/json")
    {
        return ApiFailure(response.status(), "invalid request".into()).into_response();
    }
    response
}

pub async fn no_index(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        HeaderName::from_static("x-robots-tag"),
        HeaderValue::from_static("noindex, nofollow, noarchive, nosnippet, noimageindex"),
    );
    response
}
