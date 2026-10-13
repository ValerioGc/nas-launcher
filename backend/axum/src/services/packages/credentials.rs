//! The shared token is encrypted in the systemd credential store and passed only to the
//! service. Development builds can read a local token through `NAS_LAUNCHER_TOKEN_PATH`.

use crate::{
    http::error::ApiFailure,
    services::database,
    utils::{self, Mode},
};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::io::AsyncWriteExt;

pub const LAUNCHER_CREDENTIAL: &str = "nas-os.launcher-token";

const TOKEN_UNIT: &str = "nas-os-token.service";
const TOKEN_FILE: &str = "launcher-token.new";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(20);

async fn token_in(file: PathBuf) -> Option<String> {
    let token = tokio::fs::read_to_string(file).await.ok()?;
    Some(token.trim().to_owned()).filter(|token| !token.is_empty())
}

async fn credential(name: &str) -> Option<String> {
    let directory = std::env::var("CREDENTIALS_DIRECTORY").ok()?;
    token_in(PathBuf::from(directory).join(name)).await
}

pub async fn launcher() -> Option<String> {
    if let Some(token) = credential(LAUNCHER_CREDENTIAL).await {
        return Some(token);
    }
    token_in(PathBuf::from(
        std::env::var("NAS_LAUNCHER_TOKEN_PATH").ok()?,
    ))
    .await
}

pub fn validate(token: &str) -> Result<(), ApiFailure> {
    if token.is_empty() || token.len() > 512 || !token.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(ApiFailure::bad("that does not look like a token"));
    }
    Ok(())
}

pub async fn store(token: &str) -> Result<(), ApiFailure> {
    validate(token)?;
    let real = utils::is_real(Mode::Token);
    let target = if real {
        database::beside(TOKEN_FILE)
    } else {
        std::env::var_os("NAS_LAUNCHER_TOKEN_PATH").map(PathBuf::from)
    }
    .ok_or_else(|| ApiFailure::unavailable("The token cannot be saved on this installation."))?;

    keep(&target, token)
        .await
        .map_err(|_| ApiFailure::unavailable("the token cannot be written"))?;
    if real {
        // Queued rather than awaited: the helper restarts this very process once it is done.
        if let Err(reason) =
            utils::systemctl(&["start", "--no-block", TOKEN_UNIT], COMMAND_TIMEOUT).await
        {
            let _ = tokio::fs::remove_file(&target).await;
            return Err(ApiFailure::unavailable(reason));
        }
    }
    Ok(())
}

async fn keep(path: &Path, token: &str) -> std::io::Result<()> {
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .await?;
    file.write_all(token.as_bytes()).await?;
    file.flush().await
}

#[cfg(test)]
#[path = "../../../../tests/services/packages/credentials.rs"]
mod tests;
