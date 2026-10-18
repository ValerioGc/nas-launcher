//! Lifecycle management for applications with background work.
//!
//! Applications declare `run.background` and report active work through
//! [`crate::http::app_channel`]. The launcher persists this state, starts active
//! applications at boot, and keeps them running after their windows close.
//! Background applications use the reduced priority configured by `services::grants`.

use crate::{
    http::error::ApiFailure,
    services::{
        lifecycle::{self, Action},
        manifest, registry,
    },
    utils::{self, Mode},
};
use sqlx::SqlitePool;

/// Reported background-work state of an application.
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

/// Starts applications with persisted background work sequentially at boot.
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
#[path = "../../tests/services/background.rs"]
mod tests;
