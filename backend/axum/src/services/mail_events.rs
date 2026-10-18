//! Mail events for sign-in failures, NAS updates, and application releases.
//!
//! Applications declare a versioned `mail-events` capability with an internal
//! endpoint. The launcher starts the application when needed and posts each event
//! as JSON over its socket. Internal endpoints are not exposed by the browser proxy.
//! The receiving application applies its own delivery settings.

use crate::{
    apps::services::nas_system_settings::{
        system_updates::{self, Level, Package},
        updates,
    },
    http::proxy,
    services::{
        lifecycle::{self, Action},
        manifest::{self, Manifest},
        notifications::{self, Notice, Tone},
        session::Session,
    },
    utils::{self, now_ms, Mode},
};
use axum::{
    body::{Body, Bytes},
    extract::Request,
    http::{header, Method, StatusCode},
};
use http_body_util::BodyExt;
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

pub const CAPABILITY: &str = "mail-events";
const DELIVERY_TIMEOUT: Duration = Duration::from_secs(30);
const REFRESH_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const WATCH_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Serialize)]
struct Event<'a> {
    key: &'a str,
    title: &'a str,
    message: &'a str,
    params: &'a BTreeMap<String, String>,
    at: i64,
}

pub(crate) fn is_internal_endpoint(endpoint: &str) -> bool {
    endpoint.len() <= 256
        && endpoint.starts_with("/internal/")
        && !endpoint.contains("..")
        && endpoint
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/-_.".contains(c))
}

/// The application that receives the events and its endpoint, when one is installed.
pub fn receiver(manifests: &[Manifest]) -> Option<(&Manifest, &str)> {
    manifests.iter().find_map(|manifest| {
        manifest
            .provides
            .iter()
            .find(|offered| {
                offered.capability == CAPABILITY
                    && offered.version == 1
                    && is_internal_endpoint(&offered.endpoint)
            })
            .map(|offered| (manifest, offered.endpoint.as_str()))
    })
}

/// Notification payload posted to the mail application.
pub fn body(notice: &Notice, at: i64) -> Vec<u8> {
    serde_json::to_vec(&Event {
        key: notice.key,
        title: &notice.title,
        message: &notice.message,
        params: &notice.params,
        at,
    })
    .unwrap_or_default()
}

/// Dispatches a mail event without blocking the operation that produced it.
pub fn forward(notice: &Notice) {
    let body = body(notice, now_ms());
    tokio::spawn(async move {
        if let Err(reason) = deliver(body).await {
            eprintln!("mail event not delivered: {reason}");
        }
    });
}

async fn deliver(body: Vec<u8>) -> Result<(), String> {
    let manifests = manifest::all().await;
    let Some((app, endpoint)) = receiver(&manifests) else {
        return Ok(());
    };
    let (status, _) = call(app, Method::POST, endpoint, body).await?;
    if status.is_success() {
        Ok(())
    } else {
        Err(format!("{} answered {status}", app.id))
    }
}

/// Mail capability status needed by account settings.
#[derive(Debug, Default, Serialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MailStatus {
    /// The application that sends the mails is installed.
    pub installed: bool,
    /// An SMTP server and sender are configured for verification-code delivery.
    pub sender: bool,
}

#[derive(Deserialize)]
struct ProviderStatus {
    #[serde(default)]
    sender: bool,
}

/// Queries mail delivery capability, starting the application when needed.
pub async fn status() -> MailStatus {
    let manifests = manifest::all().await;
    let Some((app, _)) = receiver(&manifests) else {
        return MailStatus::default();
    };
    let sender = match call(app, Method::GET, "/api/status", Vec::new()).await {
        Ok((status, body)) if status.is_success() => {
            serde_json::from_slice::<ProviderStatus>(&body).is_ok_and(|answer| answer.sender)
        }
        Ok(_) => false,
        Err(reason) => {
            eprintln!("the status of the mail application is unknown: {reason}");
            false
        }
    };
    MailStatus {
        installed: true,
        sender,
    }
}

#[derive(Deserialize)]
struct ProviderError {
    error: String,
}

/// Sends an account verification code and waits for the mail application result.
pub async fn send_code(to: &str, purpose: &str, code: &str) -> Result<(), (StatusCode, String)> {
    let manifests = manifest::all().await;
    let Some((app, endpoint)) = receiver(&manifests) else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "the Email app is not installed".to_owned(),
        ));
    };
    let notice = Notice::new("accountCode", "Account code", "A code of the account.")
        .with("to", to)
        .with("purpose", purpose)
        .with("code", code);
    let (status, answer) = call(app, Method::POST, endpoint, body(&notice, now_ms()))
        .await
        .map_err(|reason| (StatusCode::SERVICE_UNAVAILABLE, reason))?;
    if status.is_success() {
        return Ok(());
    }
    let reason = serde_json::from_slice::<ProviderError>(&answer).map_or_else(
        |_| format!("{} answered {status}", app.id),
        |error| error.error,
    );
    Err((
        if status.is_client_error() {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::BAD_GATEWAY
        },
        reason,
    ))
}

/// One request to the socket of the application, started when it is not running.
pub(crate) async fn call(
    app: &Manifest,
    method: Method,
    path: &str,
    body: Vec<u8>,
) -> Result<(StatusCode, Bytes), String> {
    let stream = match proxy::connect(&app.socket).await {
        Ok(stream) => stream,
        Err(_) => {
            lifecycle::run(&app.id, Action::Start)
                .await
                .map_err(|failure| failure.1)?;
            proxy::connect(&app.socket).await?
        }
    };
    let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .map_err(|error| format!("{} refused the connection: {error}", app.id))?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, "nas")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .map_err(|error| error.to_string())?;
    let answer = tokio::time::timeout(DELIVERY_TIMEOUT, async {
        let answer = sender.send_request(request).await?;
        let status = answer.status();
        Ok::<_, hyper::Error>((status, answer.into_body().collect().await?.to_bytes()))
    })
    .await
    .map_err(|_| format!("{} did not answer in time", app.id))?
    .map_err(|error| format!("{} did not answer: {error}", app.id))?;
    Ok(answer)
}

/// Reports available Debian updates by severity.
/// Package details are sent to the mail application while notifications remain compact.
pub fn upgrades_notice(packages: &[Package]) -> Notice {
    let count = |level: Level| {
        packages
            .iter()
            .filter(|package| package.level == level)
            .count()
            .to_string()
    };
    let security = count(Level::Security);
    Notice::new(
        "systemUpdatesAvailable",
        "System updates available",
        format!(
            "{} system updates available, {security} of them for security. They can be installed from Settings > Updates.",
            packages.len()
        ),
    )
    .with("count", packages.len().to_string())
    .with("security", security)
    .with("stable", count(Level::Stable))
    .with("other", count(Level::Other))
}

/// The list of upgrades a mail shows, one per line.
pub fn upgrades_list(packages: &[Package]) -> String {
    packages
        .iter()
        .map(|package| {
            format!(
                "{} {} -> {} ({})",
                package.id,
                package.current,
                package.version,
                package.level.id()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Refreshes the index, and announces the upgrades not announced yet. Returns what it found.
async fn check_system(db: &SqlitePool, announced: BTreeSet<String>) -> BTreeSet<String> {
    if let Err(reason) =
        utils::systemctl(&["start", "nas-os-apt@refresh.service"], REFRESH_TIMEOUT).await
    {
        eprintln!("the package index was not refreshed: {reason}");
    }
    let packages = match system_updates::upgradable().await {
        Ok(packages) => packages,
        Err(failure) => {
            eprintln!("system upgrades not read: {}", failure.1);
            return announced;
        }
    };
    let found: BTreeSet<String> = packages
        .iter()
        .map(|package| format!("{} {}", package.id, package.version))
        .collect();
    if !found.is_subset(&announced) {
        let notice = upgrades_notice(&packages);
        forward(&notice.clone().with("packages", upgrades_list(&packages)));
        notifications::record(db, "nas-system-settings", Tone::Info, notice).await;
    }
    found
}

/// While a mail application is installed, looks every few hours for a launcher update and for
/// system upgrades, so their mails do not wait for someone to open the settings.
pub async fn watch(db: SqlitePool, session: Session) {
    let mut announced = BTreeSet::new();
    loop {
        if receiver(&manifest::all().await).is_some() {
            if utils::is_real(Mode::Packages) {
                let mut data = session.0.write().await;
                if let Err(failure) = updates::check(&db, &mut data).await {
                    eprintln!("automatic launcher update check failed: {}", failure.1);
                }
            }
            if utils::is_real(Mode::Apt) {
                announced = check_system(&db, announced).await;
            }
        }
        tokio::time::sleep(WATCH_EVERY).await;
    }
}

#[cfg(test)]
#[path = "../../tests/services/mail_events.rs"]
mod tests;
