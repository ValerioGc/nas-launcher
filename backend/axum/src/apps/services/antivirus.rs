//! Security Center's optional ClamAV client. All machine changes run in a dedicated root unit.
pub mod executor;
pub mod scan_cache;
pub mod scheduler;
pub mod types;

use crate::{
    http::error::ApiFailure,
    services::{
        database,
        notifications::{self, Notice, Tone},
        settings::{self, Section},
        system_packages,
    },
    utils::{self, Mode},
};
use sqlx::SqlitePool;
use std::{sync::LazyLock, time::Duration};
use tokio::sync::Mutex;
pub use types::*;

const UNIT: &str = "nas-os-antivirus.service";
const APP: &str = "nas-security-center";
static OPERATION: LazyLock<Mutex<Operation>> = LazyLock::new(|| Mutex::new(Operation::default()));
static CHANGE: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

pub async fn load(db: &SqlitePool) -> Result<Settings, ApiFailure> {
    settings::read(db, Section::Antivirus).await
}

pub async fn status(db: &SqlitePool) -> Result<Status, ApiFailure> {
    let mut operation = OPERATION.lock().await.clone();
    if operation.phase == "installing" {
        if let Ok(text) =
            tokio::fs::read_to_string("/var/lib/nas-os-app-packages/antivirus-install.progress")
                .await
        {
            operation.progress = install_progress(&text);
        }
    }
    let report = report().await;
    if operation.phase == "scanning" {
        operation.progress = scan_progress(&report);
    }
    let installed = tokio::fs::try_exists("/usr/bin/clamscan")
        .await
        .unwrap_or(false);
    Ok(Status {
        settings: load(db).await?,
        operation,
        report,
        installed,
    })
}

/// The last report the job of the antivirus published.
pub async fn report() -> Report {
    match database::beside("antivirus.result.json") {
        Some(path) => tokio::fs::read(path)
            .await
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default(),
        None => Report::default(),
    }
}

/// Scan progress when the file list is known. ClamAV provides no total for unplanned scans.
fn scan_progress(report: &Report) -> Option<f64> {
    (report.planned > 0).then(|| (report.scanned as f64 * 100.0 / report.planned as f64).min(100.0))
}

fn install_progress(text: &str) -> Option<f64> {
    text.lines().rev().find_map(|line| {
        let mut parts = line.split(':');
        match parts.next()? {
            "pmstatus" | "dlstatus" => {}
            _ => return None,
        }
        parts.next()?;
        parts
            .next()?
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite() && (0.0..=100.0).contains(number))
    })
}

pub async fn save(db: &SqlitePool, input: Settings) -> Result<Settings, ApiFailure> {
    input.validate()?;
    let _change = CHANGE.lock().await;
    let current = load(db).await?;
    if current.enabled != input.enabled {
        return Err(ApiFailure::bad("use the antivirus enable action"));
    }
    if current.enabled {
        reserve("configuring").await?;
        let result = run_job(Job::Configure {
            settings: Box::new(input.clone()),
        })
        .await;
        finish(
            db,
            result
                .as_ref()
                .map(|_| ())
                .map_err(|error| ApiFailure::unavailable(error.1.clone())),
            false,
        )
        .await;
        result?;
    }
    settings::write(db, Section::Antivirus, &input).await?;
    scheduler::changed();
    Ok(input)
}

async fn reserve(phase: &str) -> Result<(), ApiFailure> {
    if !utils::is_real(Mode::Apps) {
        return Err(ApiFailure::unavailable(
            "Antivirus operations are not available on this installation.",
        ));
    }
    let mut operation = OPERATION.lock().await;
    if !operation.phase.is_empty() {
        return Err(ApiFailure::bad("an antivirus operation is already running"));
    }
    *operation = Operation {
        phase: phase.to_owned(),
        progress: None,
        error: None,
    };
    Ok(())
}

pub async fn toggle(db: &SqlitePool, enabled: bool) -> Result<(), ApiFailure> {
    let _change = CHANGE.lock().await;
    reserve(if enabled { "installing" } else { "disabling" }).await?;
    let db = db.clone();
    tokio::spawn(async move {
        let result = async {
            if enabled {
                system_packages::install(APP, &["clamav", "clamav-freshclam"])
                    .await
                    .map_err(ApiFailure::unavailable)?;
                OPERATION.lock().await.phase = "updating".into();
            }
            run_job(if enabled { Job::Enable } else { Job::Disable }).await?;
            let _change = CHANGE.lock().await;
            let mut config = load(&db).await?;
            config.enabled = enabled;
            run_job(Job::Configure {
                settings: Box::new(config.clone()),
            })
            .await?;
            settings::write(&db, Section::Antivirus, &config).await?;
            scheduler::changed();
            Ok(())
        }
        .await;
        if enabled && result.is_err() {
            let _ = run_job(Job::Disable).await;
        }
        finish(&db, result, false).await;
    });
    Ok(())
}

pub async fn scan(db: &SqlitePool, request: ScanRequest) -> Result<(), ApiFailure> {
    request.validate()?;
    let config = load(db).await?;
    if !config.enabled {
        return Err(ApiFailure::bad("antivirus is disabled"));
    }
    let options = config.options(request.kind).clone();
    launch(
        db,
        Job::Scan {
            request,
            options,
            auto_quarantine: config.auto_quarantine,
        },
        "scanning",
    )
    .await
}

/// Clears cached clean results so the next quick scan checks every file.
pub async fn forget_scanned(db: &SqlitePool) -> Result<(), ApiFailure> {
    if !load(db).await?.enabled {
        return Err(ApiFailure::bad("antivirus is disabled"));
    }
    launch(db, Job::Forget, "forgetting").await
}

pub async fn quarantine_action(
    db: &SqlitePool,
    action: &str,
    id: String,
) -> Result<(), ApiFailure> {
    if !identifier(&id) {
        return Err(ApiFailure::bad("invalid quarantine identifier"));
    }
    if !load(db).await?.enabled {
        return Err(ApiFailure::bad("antivirus is disabled"));
    }
    let job = match action {
        "quarantine" => Job::Quarantine { id },
        "restore" => Job::Restore { id },
        "delete" => Job::Delete { id },
        _ => return Err(ApiFailure::bad("invalid quarantine action")),
    };
    launch(db, job, "quarantine").await
}

async fn launch(db: &SqlitePool, job: Job, phase: &str) -> Result<(), ApiFailure> {
    reserve(phase).await?;
    let db = db.clone();
    let scan = matches!(job, Job::Scan { .. });
    tokio::spawn(async move {
        let result = run_job(job).await;
        finish(&db, result, scan).await;
    });
    Ok(())
}

async fn run_job(job: Job) -> Result<(), ApiFailure> {
    let path = database::beside("antivirus.request.json")
        .ok_or_else(|| ApiFailure::unavailable("no antivirus state directory"))?;
    let bytes = serde_json::to_vec(&job)
        .map_err(|_| ApiFailure::unavailable("cannot encode antivirus request"))?;
    tokio::fs::write(&path, bytes)
        .await
        .map_err(|_| ApiFailure::unavailable("cannot write antivirus request"))?;
    let started = utils::now_ms();
    let result = utils::systemctl(&["start", UNIT], Duration::from_secs(24 * 60 * 60)).await;
    let _ = tokio::fs::remove_file(path).await;
    if let Err(fallback) = result {
        let report: Option<Report> = match database::beside("antivirus.result.json") {
            Some(path) => tokio::fs::read(path)
                .await
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok()),
            None => None,
        };
        let reason = report
            .filter(|report| report.updated_at >= started)
            .and_then(|report| report.operation_error)
            .unwrap_or(fallback);
        return Err(ApiFailure::unavailable(reason));
    }
    Ok(())
}

async fn finish(db: &SqlitePool, result: Result<(), ApiFailure>, scan: bool) {
    let reason = result.err().map(|failure| failure.1);
    let report = status(db)
        .await
        .ok()
        .map(|state| state.report)
        .unwrap_or_default();

    *OPERATION.lock().await = Operation {
        phase: String::new(),
        progress: None,
        error: reason.clone(),
    };
    if scan || reason.is_some() {
        notify_scan(
            db,
            reason.map_or(Ok(()), |reason| Err(ApiFailure::unavailable(reason))),
            &report,
        )
        .await;
    }
}

async fn notify_scan(db: &SqlitePool, result: Result<(), ApiFailure>, report: &Report) {
    let reason = result.err().map(|error| error.1);
    let detected = report.found;
    let notice_reason = reason
        .clone()
        .unwrap_or_default()
        .chars()
        .take(120)
        .collect::<String>();
    let detail = reason.clone();
    let (tone, key, title, message) = if let Some(reason) = reason {
        (
            Tone::Error,
            "antivirusFailed",
            "Antivirus operation failed",
            reason,
        )
    } else if detected > 0 {
        (
            Tone::Warning,
            "antivirusThreats",
            "Antivirus alert",
            format!("{detected} threats detected."),
        )
    } else {
        (
            Tone::Success,
            "antivirusClean",
            "Antivirus scan completed",
            format!("{} files scanned. No threats detected.", report.scanned),
        )
    };
    let mut notice = Notice::new(key, title, message.chars().take(450).collect::<String>())
        .with("count", detected.to_string())
        .with("scanned", report.scanned.to_string())
        .with("reason", notice_reason);
    if let Some(detail) = detail {
        notice = notice.detail("antivirus", &detail, Some(UNIT)).await;
    }
    notifications::record(db, APP, tone, notice).await;
}

/// Reconciles updater state after a launcher restart, without installing packages on startup.
pub async fn resume(db: SqlitePool) {
    if !utils::is_real(Mode::Apps) {
        return;
    }
    if reserve("resuming").await.is_err() {
        return;
    }
    tokio::spawn(async move {
        if wait_for_operations().await {
            let report = status(&db)
                .await
                .ok()
                .map(|state| state.report)
                .unwrap_or_default();
            let result = report
                .operation_error
                .map_or(Ok(()), |reason| Err(ApiFailure::unavailable(reason)));
            finish(&db, result, true).await;
            if reserve("resuming").await.is_err() {
                return;
            }
        }
        deliver_lifecycle_notices(&db).await;
        let result = resume_configuration(&db).await;
        finish(&db, result, false).await;
    });
}

/// Waits for privileged jobs left running by the previous launcher process.
async fn wait_for_operations() -> bool {
    let interrupted: Option<Job> = match database::beside("antivirus.request.json") {
        Some(path) => tokio::fs::read(path)
            .await
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok()),
        None => None,
    };
    let requested_scan = matches!(interrupted, Some(Job::Scan { .. }));
    let mut running_scan = false;
    loop {
        let mut waiting = false;
        for unit in [UNIT, "nas-os-app-packages@nas-security-center.service"] {
            let active = tokio::process::Command::new("systemctl")
                .args(["is-active", unit])
                .output()
                .await
                .ok()
                .is_some_and(|output| {
                    matches!(
                        String::from_utf8_lossy(&output.stdout).trim(),
                        "active" | "activating" | "deactivating"
                    )
                });
            waiting |= active;
            if unit == UNIT && active && requested_scan {
                running_scan = true;
            }
        }
        if !waiting {
            break;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    running_scan
}

async fn resume_configuration(db: &SqlitePool) -> Result<(), ApiFailure> {
    let enabled = load(db).await.map(|config| config.enabled).unwrap_or(false);
    let installed = tokio::fs::try_exists("/usr/bin/freshclam")
        .await
        .unwrap_or(false);
    if enabled || installed {
        run_job(Job::Resume { enabled }).await?;
    }
    if enabled {
        let config = load(db).await?;
        run_job(Job::Configure {
            settings: Box::new(config),
        })
        .await?;
    }
    Ok(())
}

async fn deliver_lifecycle_notices(db: &SqlitePool) {
    let Some(path) = database::beside("antivirus.notices.json") else {
        return;
    };
    let notices: Vec<LifecycleNotice> = tokio::fs::read(path)
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let Some(marker) = database::beside("antivirus.notified") else {
        return;
    };
    let previous = tokio::fs::read_to_string(&marker)
        .await
        .ok()
        .and_then(|text| text.parse::<i64>().ok())
        .unwrap_or_default();
    let mut latest = previous;
    for notice in notices
        .into_iter()
        .filter(|notice| notice.at > previous)
        .take(20)
    {
        let result = notice
            .error
            .map_or(Ok(()), |reason| Err(ApiFailure::unavailable(reason)));
        notify_scan(
            db,
            result,
            &Report {
                found: notice.found,
                scanned: notice.scanned,
                ..Report::default()
            },
        )
        .await;
        latest = latest.max(notice.at);
    }
    let _ = tokio::fs::write(marker, latest.to_string()).await;
}

#[cfg(test)]
#[path = "../../../tests/apps/services/antivirus.rs"]
mod tests;
