use super::*;
use crate::{
    services::database,
    widgets::services::tasks::{NewTask, TaskState, TaskUpdate},
};

async fn copying(db: &SqlitePool, id: &str, name: &str) {
    tasks::register(
        db,
        NewTask {
            id: id.to_owned(),
            source: "nas-file-manager".into(),
            name: name.to_owned(),
            detail: String::new(),
            progress: None,
            pausable: false,
            cancellable: false,
        },
    )
    .await
    .expect("task");
}

#[test]
fn reads_the_jobs_of_root_among_the_units_systemd_lists() {
    let listed = "nas-os-apt@upgrade.service loaded activating start start nas-os apt\n\
        nas-os-update.service loaded activating start start nas-os update\n\
        nas-os-app-packages@nas-mail-client.service loaded activating start start packages\n\
        nas-os-apt@refresh.service loaded activating start start nas-os apt\n";

    assert_eq!(jobs_in(listed), ["update", "apt", "packages"]);
    assert!(jobs_in("").is_empty());
    assert!(jobs_in("nas-os.service loaded active running launcher").is_empty());
}

#[tokio::test]
async fn names_the_operations_under_way_and_not_the_ended_ones() {
    let db = database::open_in_memory().await.expect("database");
    assert!(busy(&db).await.tasks.is_empty());

    copying(&db, "copy-1", "Copy of Photos").await;
    copying(&db, "copy-2", "Copy of Music").await;
    tasks::update(
        &db,
        "copy-2",
        TaskUpdate {
            state: Some(TaskState::Done),
            ..TaskUpdate::default()
        },
    )
    .await
    .expect("ended");

    let found = busy(&db).await;
    assert_eq!(found.tasks, ["Copy of Photos"]);
    assert!(!found.is_empty());
    assert!(Busy::default().is_empty());
}

#[tokio::test]
async fn restarts_or_shuts_down_when_no_job_of_root_runs() {
    let db = database::open_in_memory().await.expect("database");
    // Here the switch of the power is not real: the action is only pretended.
    let outcome = run(&db, PowerAction::Shutdown).await.expect("accepted");
    assert!(outcome.simulated);
    assert_eq!(PowerAction::parse("restart"), Some(PowerAction::Restart));
    assert_eq!(PowerAction::parse("explode"), None);
}
