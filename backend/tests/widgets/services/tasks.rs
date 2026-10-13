use super::*;

#[test]
fn only_a_running_or_paused_operation_can_still_be_asked_for_anything() {
    assert!(TaskState::Running.active());
    assert!(TaskState::Paused.active());
    assert!(!TaskState::Done.active());
    assert!(!TaskState::Failed.active());
    assert!(!TaskState::Cancelled.active());
}

#[test]
fn an_identifier_is_an_identifier_and_a_name_is_free_text() {
    assert!(validate("backup-nightly", "nas-backup-manager", "Copia notturna", "").is_ok());
    assert!(validate("", "nas-backup-manager", "Copia", "").is_err());
    assert!(validate("backup nightly", "nas-backup-manager", "Copia", "").is_err());
    assert!(validate("backup", "backup manager", "Copia", "").is_err());
    assert!(validate("backup", "nas-backup-manager", "   ", "").is_err());
    assert!(validate("backup", "nas-backup-manager", "Copia", &"x".repeat(501)).is_err());
}

fn operation(id: &str, name: &str, detail: &str) -> NewTask {
    NewTask {
        id: id.to_owned(),
        source: "backup".to_owned(),
        name: name.to_owned(),
        detail: detail.to_owned(),
        progress: None,
        pausable: true,
        cancellable: true,
    }
}

async fn finish(db: &SqlitePool, id: &str, state: TaskState) {
    update(
        db,
        id,
        TaskUpdate {
            state: Some(state),
            ..Default::default()
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn finishing_limits_only_completed_operations_to_thirty() {
    let db = crate::services::database::open_in_memory().await.unwrap();
    for n in 0..35 {
        let id = format!("active-{n}");
        register(&db, operation(&id, "Copy", "running target"))
            .await
            .unwrap();
        if n % 2 == 0 {
            request(&db, &id, "pause").await.unwrap();
        }
    }
    for n in 0..31 {
        let id = format!("finished-{n}");
        register(&db, operation(&id, "Copy", &format!("target-{n}")))
            .await
            .unwrap();
        if n == 30 {
            assert!(get(&db, "finished-0").await.is_ok());
        }
        finish(
            &db,
            &id,
            [TaskState::Done, TaskState::Failed, TaskState::Cancelled][n % 3],
        )
        .await;
    }
    let rows = list(&db).await.unwrap();
    assert_eq!(rows.iter().filter(|row| row.state.active()).count(), 35);
    assert_eq!(rows.iter().filter(|row| !row.state.active()).count(), 30);
    assert!(get(&db, "finished-0").await.is_err());
    assert!(get(&db, "finished-30").await.is_ok());
}

#[tokio::test]
async fn a_repeated_completion_replaces_only_identical_finished_operations() {
    let db = crate::services::database::open_in_memory().await.unwrap();
    for (id, detail) in [("old", "Data-A"), ("other-target", "Data-B")] {
        register(&db, operation(id, "Backup", detail))
            .await
            .unwrap();
        finish(&db, id, TaskState::Done).await;
    }
    register(
        &db,
        NewTask {
            source: "other-app".to_owned(),
            ..operation("other-source", "Backup", "Data-A")
        },
    )
    .await
    .unwrap();
    finish(&db, "other-source", TaskState::Done).await;
    register(&db, operation("active", "Backup", "Data-A"))
        .await
        .unwrap();
    register(&db, operation("new", "Backup", "Data-A"))
        .await
        .unwrap();
    assert!(get(&db, "old").await.is_ok());
    finish(&db, "new", TaskState::Failed).await;
    assert!(get(&db, "old").await.is_err());
    for id in ["new", "active", "other-target", "other-source"] {
        assert!(get(&db, id).await.is_ok(), "{id}");
    }
    register(&db, operation("cancelled", "Backup", "Data-A"))
        .await
        .unwrap();
    request(&db, "cancelled", "stop").await.unwrap();
    assert!(get(&db, "new").await.is_err());
    assert!(get(&db, "active").await.is_ok());
    assert_eq!(
        get(&db, "cancelled").await.unwrap().state,
        TaskState::Cancelled
    );
}

#[tokio::test]
async fn startup_pruning_cleans_persisted_history_without_removing_active_rows() {
    use crate::services::database;
    let path = std::env::temp_dir().join(format!(
        "nas-task-retention-{}-{}.db",
        std::process::id(),
        now_ms()
    ));
    let db = database::open(&path).await.unwrap();
    register(&db, operation("active", "Copy", "target-34"))
        .await
        .unwrap();
    for n in 0..35 {
        sqlx::query(
            "INSERT INTO tasks (id, source, name, detail, state, created_at, updated_at)
                     VALUES (?, 'backup', 'Copy', ?, 'done', ?, ?)",
        )
        .bind(format!("past-{n}"))
        .bind(format!("target-{n}"))
        .bind(now_ms())
        .bind(now_ms())
        .execute(&db)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO tasks (id, source, name, detail, state, created_at, updated_at)
                 VALUES ('duplicate', 'backup', 'Copy', 'target-34', 'failed', ?, ?)",
    )
    .bind(now_ms())
    .bind(now_ms())
    .execute(&db)
    .await
    .unwrap();
    db.close().await;
    let db = database::open(&path).await.unwrap();
    prune(&db).await;
    let rows = list(&db).await.unwrap();
    assert_eq!(rows.iter().filter(|row| !row.state.active()).count(), 30);
    assert_eq!(rows.iter().filter(|row| row.state.active()).count(), 1);
    assert!(get(&db, "duplicate").await.is_ok());
    assert!(get(&db, "past-34").await.is_err());
    assert!(get(&db, "past-0").await.is_err());
    db.close().await;
    std::fs::remove_file(path).unwrap();
}
