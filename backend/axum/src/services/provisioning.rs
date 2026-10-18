//! Post-wizard installation of applications marked `preinstall` in the catalogue.
//! The wizard provisions credentials, the terminal account, and data volumes before
//! transferring control to the launcher.

use crate::{
    http::error::ApiFailure,
    services::{
        desktop, manifest,
        packages::{self, catalogue},
    },
    utils::now_ms,
};
use serde::Serialize;
use sqlx::SqlitePool;
use tokio::sync::Mutex;

const APP_PREFIX: &str = "app:";

/// One step, as the page that waits for them lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct Step {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub state: String,
    pub detail: String,
}

impl Step {
    fn finished(&self) -> bool {
        self.state == "done" || self.state == "failed"
    }
}

/// Serializes provisioning requests from the wizard and launcher startup.
static RUNNING: Mutex<()> = Mutex::const_new(());

pub async fn steps(db: &SqlitePool) -> Result<Vec<Step>, ApiFailure> {
    sqlx::query_as("SELECT id, kind, name, state, detail FROM setup_steps ORDER BY position")
        .fetch_all(db)
        .await
        .map_err(|_| ApiFailure::unavailable("the setup steps cannot be read"))
}

pub fn finished(steps: &[Step]) -> bool {
    steps.iter().all(Step::finished)
}

/// Records catalogue applications for installation after wizard completion.
pub async fn plan(db: &SqlitePool) -> Result<(), ApiFailure> {
    let planned = catalogue::read(db)
        .await
        .into_iter()
        .filter(|app| app.preinstall)
        .map(|app| (format!("{APP_PREFIX}{}", app.id), app.name));
    for (position, (id, name)) in (0_i64..).zip(planned) {
        sqlx::query(
            "INSERT OR REPLACE INTO setup_steps (id, position, kind, name, state, updated_at)
             VALUES (?, ?, 'app', ?, 'pending', ?)",
        )
        .bind(id)
        .bind(position)
        .bind(name)
        .bind(now_ms())
        .execute(db)
        .await
        .map_err(|_| ApiFailure::unavailable("the setup steps cannot be written"))?;
    }
    Ok(())
}

pub async fn mark(db: &SqlitePool, id: &str, state: &str, detail: &str) {
    let written =
        sqlx::query("UPDATE setup_steps SET state = ?, detail = ?, updated_at = ? WHERE id = ?")
            .bind(state)
            .bind(detail)
            .bind(now_ms())
            .bind(id)
            .execute(db)
            .await;
    if let Err(error) = written {
        eprintln!("setup step {id} not updated: {error}");
    }
}

/// Runs whatever is left, in order. Called after the wizard and at every start of the launcher.
pub async fn resume(db: SqlitePool) {
    let _running = RUNNING.lock().await;
    let pending: Vec<Step> = steps(&db)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|step| !step.finished())
        .collect();
    for step in pending {
        let outcome = run(&db, &step).await;
        match &outcome {
            Ok(detail) => mark(&db, &step.id, "done", detail).await,
            Err(reason) => {
                mark(&db, &step.id, "failed", reason).await;
                eprintln!("setup step {} failed: {reason}", step.id);
            }
        }
    }
}

async fn run(db: &SqlitePool, step: &Step) -> Result<String, String> {
    // Migrates legacy provisioning steps now handled by the separate setup wizard.
    let Some(id) = step.id.strip_prefix(APP_PREFIX) else {
        return Ok("done by the wizard".to_owned());
    };
    if manifest::read(id).await.is_some() {
        desktop::add_app(db, id)
            .await
            .map_err(|error| error.1.to_string())?;
        return Ok("already installed".to_owned());
    }
    mark(db, &step.id, "running", "").await;
    packages::install_for_setup(db, id)
        .await
        .map(|()| String::new())
}

#[cfg(test)]
#[path = "../../tests/services/provisioning.rs"]
mod tests;
