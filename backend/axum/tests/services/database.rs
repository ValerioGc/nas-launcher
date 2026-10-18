use super::*;

#[tokio::test]
async fn a_new_desktop_has_the_system_applications_and_keeps_a_manual_shortcut_on_reopening() {
    let pool = open_in_memory().await.expect("database");
    let targets = column(
        &pool,
        "SELECT target FROM desktop_shortcuts ORDER BY position",
    )
    .await;
    assert_eq!(
        targets,
        [
            "nas-app-manager",
            "nas-global-search",
            "nas-notification-manager",
            "nas-system-settings"
        ]
    );
    crate::services::registry::record(
        &pool,
        &crate::services::registry::Installed {
            id: "nas-system-terminal".to_owned(),
            name: "Terminal".to_owned(),
            info: String::new(),
            version: "0.1.0".to_owned(),
        },
    )
    .await
    .expect("external terminal installed");
    crate::services::desktop::add_app(&pool, "nas-system-terminal")
        .await
        .expect("manual shortcut");
    migrate(&pool).await.expect("reopen migrations");
    assert!(column(
        &pool,
        "SELECT target FROM desktop_shortcuts ORDER BY position"
    )
    .await
    .contains(&"nas-system-terminal".to_owned()));
}

async fn column(pool: &SqlitePool, query: &'static str) -> Vec<String> {
    sqlx::query_scalar(query)
        .fetch_all(pool)
        .await
        .expect("read")
}

#[tokio::test]
async fn a_copy_made_while_the_database_is_in_use_opens_and_is_whole() {
    let directory = std::env::temp_dir().join(format!("nas-os-copy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let pool = open(&directory.join("launcher.db"))
        .await
        .expect("database");
    sqlx::query("INSERT INTO notifications (source, title, message, tone, created_at) VALUES ('nas-system-settings', 'Saved', '', 'info', 0)")
        .execute(&pool)
        .await
        .expect("row");

    let mut writing = pool.begin().await.expect("transaction");
    sqlx::query("INSERT INTO notifications (source, title, message, tone, created_at) VALUES ('nas-app-manager', 'Pending', '', 'info', 0)")
        .execute(&mut *writing)
        .await
        .expect("pending row");

    let target = directory.join("copies/launcher.db");
    copy(&pool, &target).await.expect("copied");
    copy(&pool, &target)
        .await
        .expect("copied again over the first");
    writing.rollback().await.expect("rollback");

    let copied = SqlitePoolOptions::new()
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&target)
                .read_only(true),
        )
        .await
        .expect("the copy opens");
    let check: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&copied)
        .await
        .expect("checked");
    assert_eq!(check, "ok");
    assert_eq!(
        column(&copied, "SELECT title FROM notifications").await,
        ["Saved"]
    );
    let mode = std::fs::metadata(&target)
        .expect("copy")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, FILE_MODE);
    assert!(!directory.join("copies/launcher.db.new").exists());
    let _ = std::fs::remove_dir_all(&directory);
}
