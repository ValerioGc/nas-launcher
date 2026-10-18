//! Authenticated Unix socket for application processes.
//!
//! The application sandbox blocks launcher access through loopback. This socket
//! accepts connections from the launcher group and identifies each application from
//! its systemd cgroup. Routes are defined in [`crate::controllers::app_channel`].

use crate::{http::error::ApiFailure, http::state::AppState};
use axum::{
    extract::{connect_info::Connected, ConnectInfo, Request},
    middleware::Next,
    response::{IntoResponse, Response},
    serve::IncomingStream,
};
use std::{os::unix::fs::PermissionsExt, path::PathBuf};
use tokio::net::UnixListener;

/// The application a request came from, for the handlers behind the socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Caller(pub String);

/// The process at the other end of a connection, as the kernel reports it.
#[derive(Debug, Clone, Copy)]
pub struct Peer(Option<i32>);

impl Connected<IncomingStream<'_, UnixListener>> for Peer {
    fn connect_info(stream: IncomingStream<'_, UnixListener>) -> Self {
        Self(stream.io().peer_cred().ok().and_then(|cred| cred.pid()))
    }
}

/// Resolves the application socket path from `NAS_LAUNCHER_SOCKET` or build defaults.
/// Development uses the database directory. Production uses the launcher runtime directory.
pub fn socket_path() -> PathBuf {
    if let Ok(configured) = std::env::var("NAS_LAUNCHER_SOCKET") {
        return PathBuf::from(configured);
    }
    if cfg!(debug_assertions) {
        if let Some(beside) = crate::services::database::beside("launcher.sock") {
            return beside;
        }
    }
    PathBuf::from("/run/nas-os/launcher/apps.sock")
}

/// The identifier of the application a cgroup belongs to, or nothing for any other process.
pub fn application_of(cgroup: &str) -> Option<String> {
    let unit = cgroup
        .lines()
        .find_map(|line| line.strip_prefix("0::"))?
        .rsplit('/')
        .find_map(|segment| segment.strip_prefix("nas-app@")?.strip_suffix(".service"))?;
    let valid = !unit.is_empty()
        && unit.len() <= 64
        && unit
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
    valid.then(|| unit.to_owned())
}

/// Lets a request through only from a process of an application, and says which one.
pub async fn identify(
    ConnectInfo(Peer(pid)): ConnectInfo<Peer>,
    mut request: Request,
    next: Next,
) -> Response {
    let cgroup = match pid {
        Some(pid) => tokio::fs::read_to_string(format!("/proc/{pid}/cgroup"))
            .await
            .unwrap_or_default(),
        None => String::new(),
    };
    let Some(id) = application_of(&cgroup) else {
        return ApiFailure::forbidden("only an application of the NAS may use this socket")
            .into_response();
    };
    request.extensions_mut().insert(Caller(id));
    next.run(request).await
}

/// Opens the socket to the launcher's group and serves the routes of the applications on it. A
/// socket that cannot be opened leaves the launcher running, without this door.
pub async fn serve(state: AppState) {
    let path = socket_path();
    let _ = tokio::fs::remove_file(&path).await;
    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("cannot open {}: {error}", path.display());
            return;
        }
    };
    // Connecting to a unix socket takes write permission, and the umask of the launcher denies it
    // to the group.
    if let Err(error) =
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o660)).await
    {
        eprintln!(
            "cannot open {} to the applications: {error}",
            path.display()
        );
        return;
    }
    println!("applications reach the launcher on {}", path.display());
    let router = crate::http::routes::build_app_router(state);
    if let Err(error) = axum::serve(
        listener,
        router.into_make_service_with_connect_info::<Peer>(),
    )
    .await
    {
        eprintln!("the socket of the applications closed: {error}");
    }
}

#[cfg(test)]
#[path = "../../tests/http/app_channel.rs"]
mod tests;
