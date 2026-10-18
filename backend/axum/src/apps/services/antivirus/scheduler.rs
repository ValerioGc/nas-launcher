use super::{load, scan, ScanRequest, Schedule};
use crate::{
    apps::services::nas_system_settings::power::schedule::next_elapse,
    services::notifications::{self, Notice, Tone},
    utils::now_ms,
};
use sqlx::SqlitePool;
use std::{sync::LazyLock, time::Duration};
use tokio::sync::Notify;

static CHANGED: LazyLock<Notify> = LazyLock::new(Notify::new);
pub fn changed() {
    CHANGED.notify_one();
}

pub async fn run(db: SqlitePool) {
    loop {
        let upcoming = upcoming(&db).await;
        let delay = upcoming
            .first()
            .map(|(at, _)| (at * 1000 - now_ms()).max(0) as u64);
        tokio::select! {
            _ = CHANGED.notified() => continue,
            _ = tokio::time::sleep(Duration::from_millis(delay.unwrap_or(60_000).min(60_000))) => {}
        }
        for (at, job) in upcoming
            .into_iter()
            .filter(|(at, _)| *at * 1000 <= now_ms())
        {
            dispatch(&db, at, job).await;
        }
    }
}

async fn upcoming(db: &SqlitePool) -> Vec<(i64, Schedule)> {
    let config = load(db).await.unwrap_or_default();
    let mut upcoming = Vec::new();
    if config.enabled && config.scheduler_enabled {
        for job in config.schedules.into_iter().filter(|job| job.enabled) {
            let day = if job.day == "*" { "*-*-*" } else { &job.day };
            if let Some(at) = next_elapse(&format!("{day} {}:00", job.time)).await {
                upcoming.push((at, job));
            }
        }
    }
    upcoming.sort_by_key(|(at, _)| *at);
    upcoming
}

async fn dispatch(db: &SqlitePool, at: i64, job: Schedule) {
    // Revalidates the schedule after waking to discard disabled or outdated work.
    let current = load(db).await.unwrap_or_default();
    if !current.enabled || !current.scheduler_enabled {
        return;
    }
    let Some(saved) = current.schedules.iter().find(|saved| {
        saved.id == job.id && saved.enabled && saved.time == job.time && saved.day == job.day
    }) else {
        return;
    };
    if let Err(error) = scan(
        db,
        ScanRequest {
            kind: saved.kind,
            paths: saved.paths.clone(),
            skip_unchanged: false,
        },
    )
    .await
    {
        let notice = Notice::new(
            "antivirusJobSkipped",
            "Antivirus job not run",
            error.1.chars().take(450).collect::<String>(),
        )
        .with("reason", error.1.chars().take(120).collect::<String>())
        .with("job", job.id)
        .with("at", at.to_string())
        .detail("antivirus-schedule", &error.1, None)
        .await;
        notifications::record(db, "nas-security-center", Tone::Warning, notice).await;
    }
}

#[cfg(test)]
#[path = "../../../../tests/apps/services/antivirus/scheduler.rs"]
mod tests;
