use super::*;

fn rule(day: PowerDay, time: &str, action: RuleAction) -> PowerRule {
    PowerRule {
        day,
        time: time.to_owned(),
        action,
        active: true,
    }
}

#[test]
fn writes_each_rule_as_a_calendar_of_systemd() {
    assert_eq!(
        calendar(&rule(PowerDay::Everyday, "23:30", RuleAction::Shutdown)),
        "*-*-* 23:30:00"
    );
    assert_eq!(
        calendar(&rule(PowerDay::Weekdays, "07:00", RuleAction::Start)),
        "Mon..Fri 07:00:00"
    );
    assert_eq!(
        calendar(&rule(PowerDay::Weekends, "09:15", RuleAction::Restart)),
        "Sat,Sun 09:15:00"
    );
    assert_eq!(
        calendar(&rule(PowerDay::Sunday, "00:00", RuleAction::Shutdown)),
        "Sun 00:00:00"
    );
}

#[test]
fn sleeps_until_a_minute_before_the_next_action_and_never_too_long() {
    assert_eq!(pause(None), Duration::from_secs(15 * 60));
    assert_eq!(pause(Some(61)), Duration::from_secs(1));
    assert_eq!(pause(Some(10 * 60)), Duration::from_secs(9 * 60));
    assert_eq!(pause(Some(24 * 60 * 60)), Duration::from_secs(15 * 60));
}

#[test]
fn wakes_for_the_first_start_and_acts_on_the_first_other_action() {
    let events = [
        Event {
            at: 10,
            action: RuleAction::Restart,
        },
        Event {
            at: 20,
            action: RuleAction::Start,
        },
        Event {
            at: 30,
            action: RuleAction::Shutdown,
        },
    ];

    assert_eq!(next_start(&events), Some(20));
    assert_eq!(next_action(&events).map(|event| event.at), Some(10));
    assert_eq!(next_start(&events[..1]), None);
    assert!(next_action(&events[1..2]).is_none());
}

#[tokio::test]
async fn computes_the_next_elapse_in_the_nas_timezone() {
    let next = next_elapse("*-*-* 03:00:00")
        .await
        .expect("systemd calendar timestamp");
    assert!(next > now_ms() / 1000);
}

#[tokio::test]
async fn reads_the_next_events_of_the_active_rules_from_the_calendar() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("database");
    assert!(upcoming(&db).await.is_empty());

    let mut inactive = rule(PowerDay::Weekdays, "07:00", RuleAction::Start);
    inactive.active = false;
    nas_system_settings::save_power(
        &db,
        nas_system_settings::Power {
            enabled: true,
            screen_off_minutes: 5,
            rules: vec![
                rule(PowerDay::Everyday, "23:30", RuleAction::Shutdown),
                rule(PowerDay::Sunday, "08:00", RuleAction::Start),
                inactive,
            ],
        },
    )
    .await
    .expect("saved");

    let events = upcoming(&db).await;
    // Without systemd-analyze on the machine there is no calendar, and nothing is planned.
    if !events.is_empty() {
        assert_eq!(events.len(), 2);
        assert!(events.windows(2).all(|pair| pair[0].at <= pair[1].at));
        assert!(events.iter().all(|event| event.at > now_ms() / 1000));
    }
    assert!(next_elapse("not a calendar").await.is_none());
}

#[tokio::test]
async fn runs_a_planned_action_as_the_power_settings_say() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("database");

    execute(&db, RuleAction::Start, None).await;
    execute(&db, RuleAction::Restart, Some(1_900_000_000)).await;

    let simulated: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE key = 'powerSimulated'")
            .fetch_one(&db)
            .await
            .expect("notifications");
    assert_eq!(simulated, 1);
    rules_changed();
    assert!(rest(Duration::from_secs(5)).await);
}

#[tokio::test]
async fn waits_for_the_operations_under_way_then_interrupts_them() {
    use crate::widgets::services::tasks::{self, NewTask};
    let db = crate::services::database::open_in_memory()
        .await
        .expect("database");
    let count = |key: &'static str| {
        let db = db.clone();
        async move {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM notifications WHERE key = ?")
                .bind(key)
                .fetch_one(&db)
                .await
                .expect("notifications")
        }
    };

    // Nothing under way: the action goes at once and nobody is told.
    assert!(
        wait_for_work(
            &db,
            PowerAction::Shutdown,
            (WORK_CHECK, Duration::MAX),
            |_| async { panic!("idle actions must not wait") }
        )
        .await
    );
    assert_eq!(count("powerPostponed").await, 0);
    assert_eq!(count("powerInterrupting").await, 0);

    tasks::register(
        &db,
        NewTask {
            id: "copy-1".into(),
            source: "nas-file-manager".into(),
            name: "Copy of Photos".into(),
            detail: String::new(),
            progress: None,
            pausable: false,
            cancellable: false,
        },
    )
    .await
    .expect("task");
    // Keep the limit out of reach so database/systemctl latency cannot skip the postponement.
    // A change of the rules while waiting makes the action be planned again, not run.
    assert!(
        !wait_for_work(
            &db,
            PowerAction::Restart,
            (WORK_CHECK, Duration::MAX),
            |every| async move {
                assert_eq!(every, WORK_CHECK);
                true
            }
        )
        .await
    );
    assert_eq!(count("powerPostponed").await, 1);
    assert_eq!(count("powerInterrupting").await, 0);

    // An expired limit interrupts immediately, without depending on a short real-time deadline.
    assert!(
        wait_for_work(
            &db,
            PowerAction::Shutdown,
            (WORK_CHECK, Duration::ZERO),
            |_| async { panic!("expired actions must not wait") }
        )
        .await
    );
    assert_eq!(count("powerPostponed").await, 1);
    assert_eq!(count("powerInterrupting").await, 1);
}
