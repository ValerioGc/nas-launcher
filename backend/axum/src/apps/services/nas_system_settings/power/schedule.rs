//! Schedules power actions using systemd calendar expressions for timezone and DST
//! handling. Wake alarms are programmed through [`wake`] before shutdown.

use crate::{
    apps::services::nas_system_settings::{
        self,
        power::{self, wake, PowerAction},
        PowerAction as RuleAction, PowerDay, PowerRule,
    },
    services::notifications::{self, Notice, Tone},
    utils::{self, now_ms, Mode},
};
use sqlx::SqlitePool;
use std::{sync::LazyLock, time::Duration};
use tokio::{process::Command, sync::Notify};

const TICK_SECONDS: i64 = 60;
const TICK: Duration = Duration::from_secs(TICK_SECONDS as u64);
/// Maximum scheduler sleep before checking clock and time-zone changes. Rule edits wake it
/// immediately.
const MAX_REST_SECONDS: i64 = 15 * 60;
/// Scheduled-shutdown wait limit and polling interval for application operations.
const WORK_LIMIT: Duration = Duration::from_secs(60 * 60);
const WORK_CHECK: Duration = Duration::from_secs(60);

static CHANGED: LazyLock<Notify> = LazyLock::new(Notify::new);

pub fn rules_changed() {
    CHANGED.notify_one();
}

async fn rest(duration: Duration) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(duration) => false,
        _ = CHANGED.notified() => true,
    }
}

/// Systemd calendar expression for a rule.
fn calendar(rule: &PowerRule) -> String {
    let days = match rule.day {
        PowerDay::Everyday => "*-*-*",
        PowerDay::Weekdays => "Mon..Fri",
        PowerDay::Weekends => "Sat,Sun",
        PowerDay::Monday => "Mon",
        PowerDay::Tuesday => "Tue",
        PowerDay::Wednesday => "Wed",
        PowerDay::Thursday => "Thu",
        PowerDay::Friday => "Fri",
        PowerDay::Saturday => "Sat",
        PowerDay::Sunday => "Sun",
    };
    format!("{days} {}:00", rule.time)
}

pub(crate) async fn next_elapse(expression: &str) -> Option<i64> {
    let calendar = Command::new("systemd-analyze")
        .args(["calendar", "--iterations=1", expression])
        .output()
        .await
        .ok()?;
    if !calendar.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&calendar.stdout);
    let utc = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("(in UTC):"))
        .or_else(|| {
            text.lines()
                .find_map(|line| line.trim().strip_prefix("Next elapse:"))
        })?
        .trim()
        .to_owned();

    let timestamp = Command::new("systemd-analyze")
        .args(["timestamp", &utc])
        .output()
        .await
        .ok()?;

    String::from_utf8_lossy(&timestamp.stdout)
        .lines()
        .find_map(|line| line.trim().strip_prefix("UNIX seconds: @"))?
        .split('.')
        .next()?
        .trim()
        .parse()
        .ok()
}

struct Event {
    at: i64,
    action: RuleAction,
}

async fn upcoming(db: &SqlitePool) -> Vec<Event> {
    let Ok(power) = nas_system_settings::load_power(db).await else {
        return Vec::new();
    };

    if !power.enabled {
        return Vec::new();
    }

    let mut events = Vec::new();
    for rule in power.rules.iter().filter(|rule| rule.active) {
        if let Some(at) = next_elapse(&calendar(rule)).await {
            events.push(Event {
                at,
                action: rule.action,
            });
        }
    }
    events.sort_by_key(|event| event.at);
    events
}

fn next_start(events: &[Event]) -> Option<i64> {
    events
        .iter()
        .find(|event| event.action == RuleAction::Start)
        .map(|event| event.at)
}

/// Schedules calendar refresh before the next action, bounded by the maximum polling interval.
fn pause(delay: Option<i64>) -> Duration {
    let seconds = delay.map_or(MAX_REST_SECONDS, |delay| {
        (delay - TICK_SECONDS).clamp(1, MAX_REST_SECONDS)
    });
    Duration::from_secs(seconds as u64)
}

fn next_action(events: &[Event]) -> Option<&Event> {
    events
        .iter()
        .find(|event| event.action != RuleAction::Start)
}

/// Waits for privileged jobs and allows application operations up to `limit`.
/// Reports waiting and interruption once. Rule changes cancel the wait so the
/// scheduler can calculate a new action.
async fn wait_for_work<Pause, Paused>(
    db: &SqlitePool,
    action: PowerAction,
    (every, limit): (Duration, Duration),
    pause: Pause,
) -> bool
where
    Pause: Fn(Duration) -> Paused,
    Paused: std::future::Future<Output = bool>,
{
    let started = tokio::time::Instant::now();
    let mut told = false;
    loop {
        let busy = power::busy(db).await;
        let late = started.elapsed() >= limit;
        if busy.jobs.is_empty() && (busy.tasks.is_empty() || late) {
            if !busy.tasks.is_empty() {
                notifications::record(
                    db,
                    "system",
                    Tone::Warning,
                    Notice::new(
                        "powerInterrupting",
                        "Operations interrupted",
                        format!(
                            "Scheduled {} after an hour of waiting: {} interrupted.",
                            action.id(),
                            busy.tasks.join(", ")
                        ),
                    )
                    .with("action", action.id())
                    .with("operations", busy.tasks.join(", ")),
                )
                .await;
            }
            return true;
        }
        if !told {
            told = true;
            let waiting: Vec<String> = busy.jobs.iter().chain(&busy.tasks).cloned().collect();
            notifications::record(
                db,
                "system",
                Tone::Info,
                Notice::new(
                    "powerPostponed",
                    "Scheduled action postponed",
                    format!(
                        "Scheduled {} postponed while {} operations end.",
                        action.id(),
                        waiting.len()
                    ),
                )
                .with("action", action.id())
                .with("count", waiting.len().to_string())
                .with("operations", waiting.join(", ")),
            )
            .await;
        }
        if pause(every).await {
            return false;
        }
    }
}

async fn execute(db: &SqlitePool, action: RuleAction, wake_at: Option<i64>) {
    let power_action = match action {
        RuleAction::Restart => PowerAction::Restart,
        RuleAction::Shutdown => PowerAction::Shutdown,
        RuleAction::Start => return,
    };
    if !wait_for_work(db, power_action, (WORK_CHECK, WORK_LIMIT), rest).await {
        return;
    }

    if action == RuleAction::Shutdown && utils::is_real(Mode::Power) {
        if let Some(at) = wake_at {
            wake::program(db, at).await;
        }
    }

    if let Err(error) = power::run(db, power_action).await {
        notifications::record(
            db,
            "system",
            Tone::Error,
            Notice::new(
                "scheduleFailed",
                "Scheduled action not run",
                format!("Scheduled {} not run ({}).", power_action.id(), error.1),
            )
            .with("action", power_action.id())
            .with("reason", error.1.clone())
            .detail("power-schedule", &error.1, None)
            .await,
        )
        .await;
    }
}

pub async fn run(db: SqlitePool) {
    // The screen of the NAS follows the same settings, from the start.
    if let Ok(power) = nas_system_settings::load_power(&db).await {
        nas_system_settings::apply_screen(power.screen_off_minutes).await;
    }
    loop {
        let events = upcoming(&db).await;
        let wake_at = next_start(&events);
        let Some((at, action)) = next_action(&events).map(|event| (event.at, event.action)) else {
            rest(pause(None)).await;
            continue;
        };

        let delay = at - now_ms() / 1000;
        if delay > TICK_SECONDS {
            rest(pause(Some(delay))).await;
            continue;
        }

        if delay > 0 && rest(Duration::from_secs(delay as u64)).await {
            continue;
        }

        execute(&db, action, wake_at).await;
        tokio::time::sleep(TICK).await;
    }
}

#[cfg(test)]
#[path = "../../../../../tests/apps/services/nas_system_settings/power/schedule.rs"]
mod tests;
