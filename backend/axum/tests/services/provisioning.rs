use super::*;
use crate::services::database;

#[tokio::test]
async fn the_standard_catalogue_plans_only_the_initial_system_applications() {
    let db = database::open_in_memory().await.expect("database");
    let catalogue: catalogue::Catalogue = serde_json::from_str(include_str!(
        "../../../../system/packages/ui_installation/files/catalogue.json"
    ))
    .expect("standard catalogue");
    catalogue::save(&db, catalogue.apps)
        .await
        .expect("catalogue saved");
    plan(&db).await.expect("plan");
    assert_eq!(
        steps(&db)
            .await
            .expect("steps")
            .iter()
            .map(|step| step.id.as_str())
            .collect::<Vec<_>>(),
        [
            "app:nas-disk-manager",
            "app:nas-log-manager",
            "app:nas-security-center",
            "app:nas-file-manager",
            "app:nas-system-monitor",
            "app:nas-system-terminal"
        ]
    );
}

async fn step(db: &SqlitePool, id: &str, kind: &str, state: &str) {
    sqlx::query(
        "INSERT INTO setup_steps (id, position, kind, name, state, updated_at)
         VALUES (?, 0, ?, ?, ?, 0)",
    )
    .bind(id)
    .bind(kind)
    .bind(id)
    .bind(state)
    .execute(db)
    .await
    .expect("step");
}

#[tokio::test]
async fn nothing_is_left_once_every_step_has_an_outcome() {
    let db = database::open_in_memory().await.expect("database");
    assert!(finished(&steps(&db).await.expect("steps")));

    step(&db, "app:photos", "app", "done").await;
    step(&db, "app:disks", "app", "pending").await;
    assert!(!finished(&steps(&db).await.expect("steps")));

    mark(&db, "app:disks", "failed", "the release has no asset").await;
    let now = steps(&db).await.expect("steps");
    assert!(finished(&now));
    assert_eq!(now[1].detail, "the release has no asset");
}

#[tokio::test]
async fn the_steps_of_an_earlier_launcher_are_left_to_the_wizard() {
    let db = database::open_in_memory().await.expect("database");
    step(&db, "token", "token", "pending").await;

    resume(db.clone()).await;

    let token = &steps(&db).await.expect("steps")[0];
    assert_eq!(token.state, "done");
    assert_eq!(token.detail, "done by the wizard");
}

#[tokio::test]
async fn an_application_that_cannot_be_installed_fails_its_step() {
    let db = database::open_in_memory().await.expect("database");
    step(&db, "app:photos", "app", "pending").await;

    resume(db.clone()).await;

    let app = &steps(&db).await.expect("steps")[0];
    assert_eq!(app.state, "failed");
    assert!(app.detail.contains("not available on this installation"));
    let notices: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notifications")
        .fetch_one(&db)
        .await
        .expect("notifications");
    assert_eq!(notices.0, 0);
}
