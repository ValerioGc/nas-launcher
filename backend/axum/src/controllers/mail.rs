//! Internal account-mail endpoint available over loopback and excluded from browser gateway
//! routes.

use crate::{
    http::{error::ApiFailure, extract::ValidJson, state::AppState},
    services::mail_events::{self, MailStatus},
};
use axum::{http::StatusCode, routing::post, Json, Router};
use serde::Deserialize;
use utoipa_axum::{router::OpenApiRouter, routes};

/// Whether the Email app is installed and can send a code to an address
#[utoipa::path(get, path = "/mail/status", responses((status = OK, body = MailStatus)))]
async fn status() -> Json<MailStatus> {
    Json(mail_events::status().await)
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(status))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Code {
    pub to: String,
    pub purpose: String,
    pub code: String,
}

impl Code {
    pub fn valid(&self) -> bool {
        (3..=254).contains(&self.to.len())
            && self.to.contains('@')
            && !self.to.chars().any(|c| c.is_control() || c.is_whitespace())
            && matches!(self.purpose.as_str(), "verify" | "reset")
            && self.code.len() == 6
            && self.code.bytes().all(|byte| byte.is_ascii_digit())
    }
}

async fn send_code(ValidJson(input): ValidJson<Code>) -> Result<StatusCode, ApiFailure> {
    if !input.valid() {
        return Err(ApiFailure::bad("the code or its address is not valid"));
    }
    mail_events::send_code(&input.to, &input.purpose, &input.code)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(|(status, reason)| ApiFailure(status, reason))
}

/// The routes only the gateway calls, on the loopback.
pub fn internal_routes() -> Router<AppState> {
    Router::new().route("/internal/mail/code", post(send_code))
}

#[cfg(test)]
#[path = "../../tests/controllers/mail.rs"]
mod tests;
