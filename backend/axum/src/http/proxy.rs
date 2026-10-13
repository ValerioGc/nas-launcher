//! Routes `/apps/<id>/…` to the unix socket of that application, on the launcher's own address.
//! Who reaches the NAS reaches its applications.
use crate::{
    http::error::ApiFailure,
    services::manifest::{self, Manifest},
};
use axum::{
    body::Body,
    extract::Request,
    http::{header, uri::PathAndQuery, HeaderMap, HeaderValue, StatusCode, Uri},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use hyper::upgrade::OnUpgrade;
use hyper_util::rt::TokioIo;
use std::{path::Path, time::Duration};
use tokio::net::UnixStream;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

const SOCKET_PATH_LIMIT: usize = 107;

const LOCAL_USER: &str = "local";

/// Where the applications are served: `/apps/<id>/`.
pub const PREFIX: &str = "/apps/";

const HOP_BY_HOP: [&str; 8] = [
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// Routes a request under `/apps/<id>/` to that application, or lets the launcher answer.
pub async fn app_proxy(request: Request, next: Next) -> Response {
    let Some(rest) = request.uri().path().strip_prefix(PREFIX) else {
        return next.run(request).await;
    };
    let (id, inside) = match rest.split_once('/') {
        Some((id, inside)) => (id.to_owned(), format!("/{inside}")),
        None => return Redirect::permanent(&format!("{PREFIX}{rest}/")).into_response(),
    };
    let Some(app) = manifest::read(&id).await else {
        return ApiFailure::missing("this application is not installed").into_response();
    };

    let prefix = format!("{PREFIX}{id}");
    let (mut parts, body) = request.into_parts();
    let query = parts
        .uri
        .query()
        .map(|query| format!("?{query}"))
        .unwrap_or_default();
    let Ok(path) = PathAndQuery::try_from(format!("{inside}{query}")) else {
        return ApiFailure::bad("invalid application path").into_response();
    };
    parts.uri = Uri::from(path);
    if let Ok(value) = HeaderValue::from_str(&prefix) {
        parts.headers.insert("x-forwarded-prefix", value);
    }

    let mut response = forward(app, Request::from_parts(parts, body)).await;
    under_prefix(response.headers_mut(), &prefix);
    response
}

/// Whether the request asks to become a WebSocket, the one upgrade the proxy carries.
fn websocket(headers: &HeaderMap) -> bool {
    let has = |name, token: &str| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(',')
                    .any(|part| part.trim().eq_ignore_ascii_case(token))
            })
    };
    has(header::UPGRADE, "websocket") && has(header::CONNECTION, "upgrade")
}

/// Joins the browser and the application once both sides have switched protocol.
async fn tunnel(browser: OnUpgrade, application: OnUpgrade) {
    if let (Ok(browser), Ok(application)) = (browser.await, application.await) {
        let _ = tokio::io::copy_bidirectional(
            &mut TokioIo::new(browser),
            &mut TokioIo::new(application),
        )
        .await;
    }
}

fn under_prefix(headers: &mut HeaderMap, prefix: &str) {
    let moved = headers
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .filter(|location| location.starts_with('/') && !location.starts_with("//"))
        .and_then(|location| HeaderValue::from_str(&format!("{prefix}{location}")).ok());
    if let Some(location) = moved {
        headers.insert(header::LOCATION, location);
    }
}

pub(crate) async fn forward(app: Manifest, mut request: Request) -> Response {
    let path = request.uri().path();

    if path == "/internal" || path.starts_with("/internal/") {
        return ApiFailure(
            StatusCode::FORBIDDEN,
            "this path is not served to the browser".into(),
        )
        .into_response();
    }

    let upgrade = websocket(request.headers()).then(|| hyper::upgrade::on(&mut request));
    if upgrade.is_none() && request.headers().contains_key(header::UPGRADE) {
        return ApiFailure::unsupported("this proxy carries WebSocket upgrades only")
            .into_response();
    }

    let stream = match connect(&app.socket).await {
        Ok(stream) => stream,
        Err(reason) => {
            return ApiFailure::unavailable(format!("{} is not answering: {reason}", app.id))
                .into_response()
        }
    };

    let (mut sender, connection) = match hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
    {
        Ok(pair) => pair,
        Err(error) => {
            return ApiFailure::unavailable(format!("{} refused the connection: {error}", app.id))
                .into_response()
        }
    };

    let named = app.id.clone();
    tokio::spawn(async move {
        if let Err(error) = connection.with_upgrades().await {
            eprintln!("the connection to {named} ended: {error}");
        }
    });

    let (mut parts, body) = request.into_parts();
    to_application(&mut parts.headers, upgrade.is_some());

    let mut answer = match sender.send_request(Request::from_parts(parts, body)).await {
        Ok(answer) => answer,
        Err(error) => {
            return ApiFailure::unavailable(format!("{} did not answer: {error}", app.id))
                .into_response()
        }
    };

    if let Some(browser) = upgrade {
        if answer.status() == StatusCode::SWITCHING_PROTOCOLS {
            let application = hyper::upgrade::on(&mut answer);
            tokio::spawn(tunnel(browser, application));
            let (mut parts, _) = answer.into_parts();
            host_only_cookies(&mut parts.headers);
            return Response::from_parts(parts, Body::empty());
        }
    }

    let (mut parts, body) = answer.into_parts();
    for name in HOP_BY_HOP {
        parts.headers.remove(name);
    }
    host_only_cookies(&mut parts.headers);
    Response::from_parts(parts, Body::new(body))
}

/// Drops the hop-by-hop headers, keeps a WebSocket upgrade and names the user to the application.
fn to_application(headers: &mut HeaderMap, upgrading: bool) {
    let protocol = headers.get(header::UPGRADE).cloned();
    for name in HOP_BY_HOP {
        headers.remove(name);
    }
    if let (true, Some(protocol)) = (upgrading, protocol) {
        headers.insert(header::CONNECTION, HeaderValue::from_static("upgrade"));
        headers.insert(header::UPGRADE, protocol);
    }

    let user = headers
        .get("x-nas-user")
        .filter(|value| {
            value.as_bytes().len() <= 25
                && !value.is_empty()
                && value
                    .as_bytes()
                    .iter()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(byte))
        })
        .cloned()
        .unwrap_or(HeaderValue::from_static(LOCAL_USER));
    headers.insert("x-nas-user", user);
}

fn host_only_cookies(headers: &mut HeaderMap) {
    let kept: Vec<HeaderValue> = headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter(|cookie| {
            cookie.to_str().is_ok_and(|text| {
                !text
                    .split(';')
                    .skip(1)
                    .any(|attribute| attribute.trim().to_ascii_lowercase().starts_with("domain"))
            })
        })
        .cloned()
        .collect();
    headers.remove(header::SET_COOKIE);
    for cookie in kept {
        headers.append(header::SET_COOKIE, cookie);
    }
}

pub(crate) async fn connect(socket: &Path) -> Result<UnixStream, String> {
    if socket.as_os_str().len() > SOCKET_PATH_LIMIT {
        return Err(format!(
            "its socket path is longer than the {SOCKET_PATH_LIMIT} bytes a unix socket allows"
        ));
    }

    match tokio::time::timeout(CONNECT_TIMEOUT, UnixStream::connect(socket)).await {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Err("it is not running".to_owned())
        }
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err("it did not accept the connection".to_owned()),
    }
}

#[cfg(test)]
#[path = "../../../tests/http/proxy.rs"]
mod tests;
