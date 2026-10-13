//! The applications that work without their window: a backup that runs at its hour, the mails of
//! the NAS. One declares `run.background` in its manifest and then says by itself, on the socket of
//! [`crate::http::app_channel`], whether it has work to do now: rules that are on, notifications to
//! send. The launcher keeps the answer in the register of the installed applications, starts those
//! that have work at boot and leaves them running when their window closes. An application with
//! nothing to do is stopped with its window, as any other.
//!
//! The application runs in its own unit as always, at the lower priority `services::grants` gives
//! it, and stops like any other: `systemctl stop nas-app@<id>`, or by turning off its work.

use crate::{
    http::error::ApiFailure,
    services::{
        lifecycle::{self, Action},
        manifest, registry,
    },
    utils::{self, Mode},
};
use sqlx::SqlitePool;

/// What an application says of itself: whether it has work to do without its window.
pub async fn declare(db: &SqlitePool, id: &str, needed: bool) -> Result<(), ApiFailure> {
    let declared = manifest::read(id).await.is_some_and(|app| app.background);
    if !declared {
        return Err(ApiFailure::forbidden(format!(
            "{id} does not declare run.background in its manifest"
        )));
    }
    match registry::set_background(db, id, needed).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(ApiFailure::missing(format!("{id} is not installed"))),
        Err(error) => Err(ApiFailure::unavailable(format!(
            "the register of the applications is not available: {error}"
        ))),
    }
}

/// Whether closing the window of an application leaves it running.
pub async fn keeps_running(db: &SqlitePool, id: &str) -> bool {
    let declared = manifest::read(id).await.is_some_and(|app| app.background);
    declared
        && registry::in_background(db)
            .await
            .is_ok_and(|ids| ids.iter().any(|item| item == id))
}

/// At boot: starts the applications that had work to do, one after the other.
pub async fn start(db: SqlitePool) {
    if !utils::is_real(Mode::Apps) {
        return;
    }
    let ids = match registry::in_background(&db).await {
        Ok(ids) => ids,
        Err(error) => {
            eprintln!("<4>the applications in background are not started: {error}");
            return;
        }
    };
    for id in ids {
        if !manifest::read(&id).await.is_some_and(|app| app.background) {
            continue;
        }
        match lifecycle::run(&id, Action::Start).await {
            Ok(()) => println!("{id} started: it has work to do in background"),
            Err(failure) => eprintln!("<4>{id} was not started in background: {}", failure.1),
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/services/background.rs"]
mod tests;
