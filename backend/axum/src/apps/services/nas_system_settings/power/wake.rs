//! Writes a wake request beside the database and asks the privileged system unit
//! to program the RTC. Failures are reported without blocking shutdown.

use crate::{
    services::{
        database,
        notifications::{self, Notice, Tone},
    },
    utils,
};
use sqlx::SqlitePool;
use std::time::Duration;

const WAKE_UNIT: &str = "nas-os-wake.service";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

fn request_path() -> Option<std::path::PathBuf> {
    database::beside("wakealarm")
}

async fn missing(db: &SqlitePool, reason: &str) {
    notifications::record(
        db,
        "system",
        Tone::Warning,
        Notice::new(
            "wakeFailed",
            "Automatic power-on not set",
            format!("Shutdown without automatic power-on ({reason})."),
        )
        .with("reason", reason)
        .detail("wake", reason, Some(WAKE_UNIT))
        .await,
    )
    .await;
}

pub async fn program(db: &SqlitePool, at: i64) {
    let Some(path) = request_path() else {
        missing(db, "no state directory for the wake request").await;
        return;
    };

    if let Err(error) = tokio::fs::write(&path, at.to_string()).await {
        missing(db, &format!("cannot write {}: {error}", path.display())).await;
        return;
    }

    match utils::systemctl(&["start", WAKE_UNIT], COMMAND_TIMEOUT).await {
        Ok(()) => {
            notifications::record(
                db,
                "system",
                Tone::Info,
                Notice::new(
                    "wakeSet",
                    "Automatic power-on set",
                    format!("Automatic power-on of the NAS scheduled for {at} (Unix time)."),
                )
                .with("wakeAt", (at * 1000).to_string()),
            )
            .await;
        }
        Err(reason) => missing(db, &reason).await,
    }
}
