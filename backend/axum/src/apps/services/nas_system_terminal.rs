//! Terminal application

use crate::{
    apps::services::nas_system_settings::is_colour,
    http::error::ApiFailure,
    services::settings::{self, Section},
    utils::{self, Mode},
};
use axum::http::StatusCode;
use futures_util::{stream, Stream};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, LazyLock, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{unix::OwnedWriteHalf, UnixStream},
    process::Command,
};

pub const JOURNAL: (&str, [&str; 5]) = (
    "journalctl",
    [
        "--follow",
        "--no-pager",
        "--output=short-iso",
        "--lines=200",
        "--quiet",
    ],
);

const MAX_LINE: usize = 4096;
const MAX_COMMAND: usize = 4096;
const MAX_SESSIONS: usize = 4;
const DONE: char = '\u{1e}';
const MARKER: &str = "__nas_code=$?; printf '\\036%s %s\\n' \"$__nas_code\" \"$PWD\"\n";

const SOCKET: &str = "/run/nas-os-terminal.sock";

type Input = Arc<tokio::sync::Mutex<OwnedWriteHalf>>;

static SESSIONS: LazyLock<Mutex<HashMap<String, Input>>> = LazyLock::new(Mutex::default);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    Session(String),
    Line(String),
    Done { code: i32, cwd: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TerminalSettings {
    pub transparency: u8,
    pub text_color: String,
    pub background_color: String,
    pub highlight_color: String,
    pub font_size: u8,
    pub line_height: f32,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            transparency: 0,
            text_color: "#d6e2ea".to_owned(),
            background_color: "#0e1922".to_owned(),
            highlight_color: "#3fd39b".to_owned(),
            font_size: 14,
            line_height: 1.5,
        }
    }
}

impl TerminalSettings {
    pub fn validate(&self) -> Result<(), ApiFailure> {
        let valid = self.transparency <= 100
            && [
                &self.text_color,
                &self.background_color,
                &self.highlight_color,
            ]
            .iter()
            .all(|colour| is_colour(colour))
            && (10..=32).contains(&self.font_size)
            && (1.0..=2.5).contains(&self.line_height);
        if valid {
            Ok(())
        } else {
            Err(ApiFailure::bad("invalid terminal settings"))
        }
    }
}

pub async fn load_settings(db: &SqlitePool) -> Result<TerminalSettings, ApiFailure> {
    settings::read(db, Section::Terminal).await
}

pub async fn save_settings(
    db: &SqlitePool,
    input: TerminalSettings,
) -> Result<TerminalSettings, ApiFailure> {
    input.validate()?;
    settings::write(db, Section::Terminal, &input).await?;
    settings::record_saved(db, Section::Terminal).await;
    Ok(input)
}

pub fn validate_command(command: &str) -> Result<(), ApiFailure> {
    if command.len() > MAX_COMMAND || command.chars().any(char::is_control) {
        return Err(ApiFailure::bad("invalid command"));
    }

    Ok(())
}

fn disabled() -> ApiFailure {
    ApiFailure::unavailable("The terminal is not available on this installation.")
}

pub fn journal() -> Result<impl Stream<Item = String>, ApiFailure> {
    if !utils::is_real(Mode::Terminal) {
        return Err(disabled());
    }

    let (program, args) = JOURNAL;
    lines(program, &args)
}

pub fn lines(program: &str, args: &[&str]) -> Result<impl Stream<Item = String>, ApiFailure> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| ApiFailure::unavailable(format!("{program} is not available: {error}")))?;

    let output = child
        .stdout
        .take()
        .ok_or_else(|| ApiFailure::unavailable(format!("{program} has no output")))?;

    let reader = BufReader::new(output).lines();
    Ok(stream::unfold(
        (reader, child),
        |(mut reader, child)| async move {
            let line = reader.next_line().await.ok()??;
            Some((clip(line), (reader, child)))
        },
    ))
}

struct Registered(String);

impl Drop for Registered {
    fn drop(&mut self) {
        if let Ok(mut sessions) = SESSIONS.lock() {
            sessions.remove(&self.0);
        }
    }
}

async fn new_id() -> Result<String, ApiFailure> {
    let mut bytes = [0u8; 16];
    tokio::fs::File::open("/dev/urandom")
        .await
        .map_err(|_| unavailable())?
        .read_exact(&mut bytes)
        .await
        .map_err(|_| unavailable())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn unavailable() -> ApiFailure {
    ApiFailure::unavailable("the terminal cannot start a shell")
}

fn socket() -> PathBuf {
    std::env::var_os("NAS_TERMINAL_SOCKET").map_or_else(|| PathBuf::from(SOCKET), PathBuf::from)
}

pub async fn open() -> Result<impl Stream<Item = TerminalEvent>, ApiFailure> {
    if !utils::is_real(Mode::Terminal) {
        return Err(disabled());
    }

    let id = new_id().await?;
    let (output, mut input) = UnixStream::connect(socket())
        .await
        .map_err(|_| {
            ApiFailure::unavailable(
                "the terminal has no shell to open: its user is not ready or its socket is off",
            )
        })?
        .into_split();

    input
        .write_all(format!("exec 2>&1\n{MARKER}").as_bytes())
        .await
        .map_err(|_| unavailable())?;

    {
        let mut sessions = SESSIONS.lock().map_err(|_| unavailable())?;
        if sessions.len() >= MAX_SESSIONS {
            return Err(ApiFailure(
                StatusCode::TOO_MANY_REQUESTS,
                "too many terminals are open".into(),
            ));
        }
        sessions.insert(id.clone(), Arc::new(tokio::sync::Mutex::new(input)));
    }

    let reader = BufReader::new(output).lines();
    let state = (reader, Registered(id.clone()));
    let lines = stream::unfold(state, |(mut reader, registered)| async move {
        let line = reader.next_line().await.ok()??;
        Some((event(line), (reader, registered)))
    });
    Ok(futures_util::StreamExt::chain(
        stream::iter([TerminalEvent::Session(id)]),
        lines,
    ))
}

/// Sends one command to the shell of a session.
pub async fn run(id: &str, command: &str) -> Result<(), ApiFailure> {
    validate_command(command)?;
    let input = SESSIONS
        .lock()
        .map_err(|_| unavailable())?
        .get(id)
        .cloned()
        .ok_or_else(|| ApiFailure::missing("no such terminal session"))?;

    let mut shell = input.lock().await;
    shell
        .write_all(format!("{command}\n{MARKER}").as_bytes())
        .await
        .map_err(|_| ApiFailure::missing("the terminal session has ended"))
}

pub fn event(line: String) -> TerminalEvent {
    if let Some(rest) = line.strip_prefix(DONE) {
        let (code, cwd) = rest.split_once(' ').unwrap_or((rest, ""));
        return TerminalEvent::Done {
            code: code.parse().unwrap_or(-1),
            cwd: cwd.to_owned(),
        };
    }
    TerminalEvent::Line(clip(line))
}

pub fn clip(line: String) -> String {
    let mut line = line.replace('\r', "");
    if line.len() > MAX_LINE {
        let mut end = MAX_LINE;
        while !line.is_char_boundary(end) {
            end -= 1;
        }
        line.truncate(end);
    }
    line
}

#[cfg(test)]
#[path = "../../../../tests/apps/services/nas_system_terminal.rs"]
mod tests;
