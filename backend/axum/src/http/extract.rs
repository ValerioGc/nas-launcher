//! JSON extraction that preserves serde validation errors in the API error format.

use crate::http::error::ApiFailure;
use axum::{
    extract::{rejection::JsonRejection, ConnectInfo, FromRequest, FromRequestParts, Request},
    http::request::Parts,
    Json,
};
use serde::de::DeserializeOwned;
use std::{
    convert::Infallible,
    net::{IpAddr, SocketAddr},
};

pub struct ValidJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiFailure;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(request, state).await {
            Ok(Json(value)) => Ok(ValidJson(value)),
            Err(rejection) => Err(ApiFailure::bad(describe(&rejection))),
        }
    }
}

fn describe(rejection: &JsonRejection) -> String {
    match rejection {
        JsonRejection::JsonDataError(error) => {
            let text = error.body_text();
            text.split_once(": ")
                .map(|(_, detail)| detail.to_owned())
                .unwrap_or(text)
        }
        JsonRejection::JsonSyntaxError(_) => "malformed JSON body".to_owned(),
        JsonRejection::MissingJsonContentType(_) => "application/json required".to_owned(),
        _ => "invalid request body".to_owned(),
    }
}

/// Optional connection address
pub struct Peer(pub Option<IpAddr>);

impl<S> FromRequestParts<S> for Peer
where
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Peer(peer_address(&parts.extensions)))
    }
}

/// Shared with the access guard, which reads the same extension from the whole request.
pub fn peer_address(extensions: &axum::http::Extensions) -> Option<IpAddr> {
    extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(peer)| peer.ip())
}

/// Set by the gateway, with the name of the account, once it has checked its password again for
/// this one request.
pub const REAUTHENTICATED_HEADER: &str = "x-nas-reauthenticated";

/// The account whose password the gateway checked again for this request: the factory reset and
/// the export and import of the configuration need it. A request that reaches the launcher without
/// it, from the gateway without a code or from anywhere else, is refused.
#[derive(Debug)]
pub struct Reauthenticated(pub String);

impl<S> FromRequestParts<S> for Reauthenticated
where
    S: Send + Sync,
{
    type Rejection = ApiFailure;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .headers
            .get(REAUTHENTICATED_HEADER)
            .and_then(|value| value.to_str().ok())
            .filter(|user| !user.is_empty())
            .map(|user| Reauthenticated(user.to_owned()))
            .ok_or_else(|| ApiFailure::forbidden("the password of the account was not confirmed"))
    }
}

#[cfg(test)]
#[path = "../../../tests/http/extract.rs"]
mod tests;
