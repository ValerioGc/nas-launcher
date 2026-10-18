//! GitHub/GitLab release checks and bounded streaming downloads.
//! API base URLs can be overridden to test package installation against a local forge.

use crate::{
    services::packages::catalogue::Provider,
    utils::{self, Mode},
};
use serde::Deserialize;
use std::{path::Path, time::Duration};
use tokio::io::{AsyncWriteExt, BufWriter};

const CHECK_TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);

const MAX_ASSET: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Asset {
    pub name: String,
    pub url: String,
}

impl Release {
    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.name == name)
    }

    pub fn asset_like(&self, name: &str) -> Option<&Asset> {
        let plain = |text: &str| {
            text.chars()
                .filter(|c| !matches!(c, '-' | '_'))
                .collect::<String>()
                .to_lowercase()
        };
        if let Some(exact) = self.asset(name) {
            return Some(exact);
        }
        let wanted = plain(name);
        let mut close = self
            .assets
            .iter()
            .filter(|asset| plain(&asset.name) == wanted);
        match (close.next(), close.next()) {
            (Some(only), None) => Some(only),
            _ => None,
        }
    }
}

fn base(provider: Provider) -> String {
    match provider {
        Provider::Github => {
            std::env::var("NAS_GITHUB_API").unwrap_or_else(|_| "https://api.github.com".to_owned())
        }
        Provider::Gitlab => std::env::var("NAS_GITLAB_API")
            .unwrap_or_else(|_| "https://gitlab.com/api/v4".to_owned()),
    }
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("nas-os/", env!("CARGO_PKG_VERSION")))
        // Release asset endpoints can redirect to the forge's download host.
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .map_err(|error| format!("cannot build the HTTP client: {error}"))
}

/// The token, when one is configured. See `packages::credentials`.
fn authorise(request: reqwest::RequestBuilder, token: Option<&str>) -> reqwest::RequestBuilder {
    match token {
        Some(token) => request.bearer_auth(token),
        None => request,
    }
}

fn for_forge<'a>(url: &str, token: Option<&'a str>) -> Option<&'a str> {
    let origin = |address: &str| reqwest::Url::parse(address).ok().map(|url| url.origin());
    let target = origin(url)?;
    let known = [Provider::Github, Provider::Gitlab]
        .into_iter()
        .any(|provider| origin(&base(provider)).as_ref() == Some(&target));
    known.then_some(token).flatten()
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Default, Deserialize)]
struct GitlabAssets {
    #[serde(default)]
    links: Vec<Asset>,
}
#[derive(Deserialize)]
struct GitlabRelease {
    tag_name: String,
    #[serde(default)]
    assets: GitlabAssets,
}

pub async fn latest(
    provider: Provider,
    repository: &str,
    token: Option<&str>,
) -> Result<Release, String> {
    latest_logged(provider, repository, token, None).await
}

pub async fn latest_logged(
    provider: Provider,
    repository: &str,
    token: Option<&str>,
    log: Option<&Path>,
) -> Result<Release, String> {
    if !utils::is_real(Mode::Packages) {
        return Err("package operations are disabled on this installation".to_owned());
    }
    if repository.is_empty() || repository.contains(['?', '#', ' ']) {
        return Err(format!("invalid repository: {repository}"));
    }

    let client = client()?;
    let url = match provider {
        Provider::Github => format!("{}/repos/{repository}/releases/latest", base(provider)),
        Provider::Gitlab => format!(
            "{}/projects/{}/releases/permalink/latest",
            base(provider),
            repository.replace('/', "%2F")
        ),
    };

    let request = authorise(client.get(&url).timeout(CHECK_TIMEOUT), token);
    trace(
        log,
        &format!(
            "GET {url}\nAuthorization: {}",
            if token.is_some() {
                "configured"
            } else {
                "not configured"
            }
        ),
    )
    .await;
    let answer = match request.send().await {
        Ok(answer) => answer,
        Err(error) => {
            let reason = format!("cannot reach {repository}: {error}");
            trace(log, &format!("REQUEST FAILED\n{reason}")).await;
            return Err(reason);
        }
    };
    let status = answer.status();
    let content_type = answer
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .to_owned();
    let body = match answer.bytes().await {
        Ok(body) => body,
        Err(error) => {
            let reason = format!("cannot read the answer from {repository}: {error}");
            trace(log, &format!("RESPONSE READ FAILED\n{reason}")).await;
            return Err(reason);
        }
    };
    let shown = String::from_utf8_lossy(&body[..body.len().min(32 * 1024)]);
    trace(
        log,
        &format!(
            "HTTP {}\nContent-Type: {content_type}\n\n{shown}",
            status.as_u16()
        ),
    )
    .await;
    if !status.is_success() {
        return Err(describe(status, repository));
    }

    let (tag, assets) = match provider {
        Provider::Github => {
            let release: GithubRelease = serde_json::from_slice(&body)
                .map_err(|error| format!("unreadable answer from {repository}: {error}"))?;
            (release.tag_name, release.assets)
        }
        Provider::Gitlab => {
            let release: GitlabRelease = serde_json::from_slice(&body)
                .map_err(|error| format!("unreadable answer from {repository}: {error}"))?;
            (release.tag_name, release.assets.links)
        }
    };
    Ok(Release {
        version: version(&tag),
        assets,
    })
}

async fn trace(path: Option<&Path>, text: &str) {
    let Some(path) = path else { return };
    let Ok(file) = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
    else {
        return;
    };
    let mut writer = BufWriter::new(file);
    let _ = writer.write_all(text.as_bytes()).await;
    let _ = writer.write_all(b"\n\n").await;
    let _ = writer.flush().await;
}

fn describe(status: reqwest::StatusCode, repository: &str) -> String {
    match status.as_u16() {
        401 | 403 => format!("{repository} refused the token"),
        404 => format!("{repository} has no release, or the token cannot see it"),
        other => format!("{repository} answered {other}"),
    }
}

pub fn version(tag: &str) -> String {
    tag.trim().trim_start_matches('v').to_owned()
}

pub fn is_newer(candidate: &str, installed: &str) -> bool {
    let parts = |value: &str| {
        value
            .split(['.', '-', '+'])
            .map(|piece| piece.parse::<u64>().ok())
            .collect::<Vec<_>>()
    };
    let (left, right) = (parts(candidate), parts(installed));

    for index in 0..left.len().max(right.len()) {
        let one = left.get(index).copied().flatten().unwrap_or(0);
        let other = right.get(index).copied().flatten().unwrap_or(0);
        if one != other {
            return one > other;
        }
    }
    false
}

pub async fn download(url: &str, destination: &Path, token: Option<&str>) -> Result<u64, String> {
    download_logged(url, destination, token, None).await
}

pub async fn download_logged(
    url: &str,
    destination: &Path,
    token: Option<&str>,
    log: Option<&Path>,
) -> Result<u64, String> {
    use futures_util::StreamExt;

    let client = client()?;
    let request = authorise(
        client
            .get(url)
            .timeout(DOWNLOAD_TIMEOUT)
            .header(reqwest::header::ACCEPT, "application/octet-stream"),
        for_forge(url, token),
    );

    trace(log, &format!("GET {url}\nAccept: application/octet-stream")).await;
    let answer = match request.send().await {
        Ok(answer) => answer,
        Err(error) => {
            let reason = format!("cannot download the package: {error}");
            trace(log, &format!("DOWNLOAD REQUEST FAILED\n{reason}")).await;
            return Err(reason);
        }
    };
    if !answer.status().is_success() {
        let status = answer.status();
        let body = answer.bytes().await.unwrap_or_default();
        let shown = String::from_utf8_lossy(&body[..body.len().min(32 * 1024)]);
        trace(log, &format!("HTTP {}\n\n{shown}", status.as_u16())).await;
        return Err(format!("the package answered {}", status.as_u16()));
    }

    trace(log, &format!("HTTP {}", answer.status().as_u16())).await;

    let expected = answer.content_length();
    if expected.is_some_and(|size| size > MAX_ASSET) {
        return Err("the package is larger than this launcher will fetch".to_owned());
    }

    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(|error| format!("cannot write {}: {error}", destination.display()))?;
    let mut written: u64 = 0;
    let mut stream = answer.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("the download stopped: {error}"))?;
        written += chunk.len() as u64;
        if written > MAX_ASSET {
            return Err("the package is larger than this launcher will fetch".to_owned());
        }
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("cannot write {}: {error}", destination.display()))?;
    }

    file.flush()
        .await
        .map_err(|error| format!("cannot write {}: {error}", destination.display()))?;
    Ok(written)
}

#[cfg(test)]
#[path = "../../../tests/services/packages/forge.rs"]
mod tests;
