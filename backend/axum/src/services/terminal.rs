//! System services for the console and the journal of the external app nas-system-terminal.

use crate::{
    apps::services::nas_system_settings::is_colour,
    http::error::ApiFailure,
    services::settings::{self, Section},
    utils::{self, Mode},
};
use futures_util::{stream, Stream};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
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
#[path = "../../../tests/services/terminal.rs"]
mod tests;
