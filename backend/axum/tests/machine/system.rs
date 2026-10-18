use crate::support::Machine;
use nas_os_axum::{
    apps::services::nas_system_settings::{
        power::wake,
        system_updates::{self, Job},
    },
    services::{
        database,
        session::{Session, SessionData},
    },
    widgets::services::tasks,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;

async fn finished(db: &sqlx::SqlitePool, task: &str) -> tasks::TaskState {
    for _ in 0..200 {
        let state = tasks::get(db, task).await.expect("task").state;
        if !matches!(state, tasks::TaskState::Running) {
            return state;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("{task} did not end");
}

#[tokio::test]
async fn lists_and_installs_the_upgrades_of_the_system() {
    let machine = Machine::get();
    let db = database::open_in_memory().await.expect("database");
    let session = Session(Arc::new(RwLock::new(SessionData::default())));

    let state = system_updates::state(&session).await.expect("state");
    assert!(state.manageable);
    assert_eq!(
        state
            .packages
            .iter()
            .map(|package| package.id.as_str())
            .collect::<Vec<_>>(),
        ["bash", "curl"]
    );

    for wrong in [Vec::new(), vec!["vim".to_owned()]] {
        let refused = system_updates::start(&db, &session, Job::Upgrade, &wrong).await;
        assert!(refused.is_err(), "{wrong:?}");
    }

    let chosen = vec!["bash".to_owned()];
    system_updates::start(&db, &session, Job::Upgrade, &chosen)
        .await
        .expect("upgrade started");
    assert!(matches!(
        finished(&db, "apt-upgrade").await,
        tasks::TaskState::Done
    ));
    let selection = database::beside("apt-upgrade").expect("selection");
    assert_eq!(
        std::fs::read_to_string(selection).expect("selection"),
        "bash"
    );
    assert!(!machine
        .calls("systemctl start nas-os-apt@upgrade.service")
        .is_empty());

    let mut updated = 0;
    for _ in 0..100 {
        updated = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM notifications WHERE key = 'systemPackagesUpdated'",
        )
        .fetch_one(&db)
        .await
        .expect("notifications");
        if updated > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(updated, 1);

    for _ in 0..100 {
        if session.0.read().await.apps.apt_job.is_none() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    session.0.write().await.apps.apt_job = Some(Job::Upgrade);
    let busy = system_updates::start(&db, &session, Job::Refresh, &[]).await;
    assert!(busy.is_err());
    session.0.write().await.apps.apt_job = None;
    system_updates::start(&db, &session, Job::Refresh, &[])
        .await
        .expect("refresh started");
    assert!(matches!(
        finished(&db, "apt-refresh").await,
        tasks::TaskState::Done
    ));
}

#[tokio::test]
async fn asks_the_root_unit_to_program_the_power_on() {
    let machine = Machine::get();
    let db = database::open_in_memory().await.expect("database");

    wake::program(&db, 1_900_000_000).await;

    let request = database::beside("wakealarm").expect("request");
    assert_eq!(
        std::fs::read_to_string(request).expect("request"),
        "1900000000"
    );
    assert!(!machine
        .calls("systemctl start nas-os-wake.service")
        .is_empty());

    let failed: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE key = 'wakeFailed'")
            .fetch_one(&db)
            .await
            .expect("notifications");
    assert_eq!(failed, 1);
}

#[tokio::test]
async fn keeps_a_copy_of_the_database_and_plans_the_power_actions_in_the_background() {
    use nas_os_axum::apps::services::nas_system_settings::power::schedule;
    let machine = Machine::get();
    let folder = machine.root.join("copied");
    let db = database::open(&folder.join("launcher.db"))
        .await
        .expect("database");

    let copying = tokio::spawn(database::keep_copy(db.clone(), folder.join("launcher.db")));
    let planning = tokio::spawn(schedule::run(db.clone()));
    let target = folder.join("copies/launcher.db");
    for _ in 0..200 {
        if target.is_file() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    copying.abort();
    planning.abort();
    let _ = copying.await;
    let _ = planning.await;
    assert!(target.is_file());
}

#[tokio::test]
async fn hands_the_minutes_of_the_screen_beside_the_database() {
    use nas_os_axum::apps::services::nas_system_settings::{self as settings, Power};
    Machine::get();
    let db = database::open_in_memory().await.expect("database");

    let old: Power = serde_json::from_str(r#"{"enabled":false,"rules":[]}"#).expect("power");
    assert_eq!(old.screen_off_minutes, settings::SCREEN_OFF_DEFAULT);
    for wrong in [0, 241] {
        let power = Power {
            screen_off_minutes: wrong,
            ..Power::default()
        };
        assert!(power.validate().is_err(), "{wrong}");
    }

    settings::save_power(
        &db,
        Power {
            screen_off_minutes: 25,
            ..Power::default()
        },
    )
    .await
    .expect("saved");
    let written = database::beside("screen-off-minutes").expect("minutes");
    let minutes: u16 = std::fs::read_to_string(written)
        .expect("minutes")
        .trim()
        .parse()
        .expect("number");

    assert!((1..=240).contains(&minutes));
}

#[tokio::test]
async fn holds_a_restart_back_while_a_job_of_root_runs() {
    let _operations = crate::support::ROOT_OPERATIONS.lock().await;
    use axum::http::StatusCode;
    use nas_os_axum::apps::services::nas_system_settings::power::{self, PowerAction};
    let machine = Machine::get();
    let db = database::open_in_memory().await.expect("database");
    let listed = machine.root.join("answers/systemctl@list-units");

    std::fs::write(
        &listed,
        "nas-os-update.service loaded activating start start nas-os update\n",
    )
    .expect("listed units");
    let busy = power::busy(&db).await;
    let refused = power::run(&db, PowerAction::Restart).await;
    std::fs::remove_file(&listed).expect("listed units");

    assert_eq!(busy.jobs, ["update"]);
    assert_eq!(refused.unwrap_err().0, StatusCode::CONFLICT);
    assert!(power::busy(&db).await.jobs.is_empty());
}
