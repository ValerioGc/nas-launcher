use super::*;
use crate::services::database;

async fn notices(db: &SqlitePool) -> Vec<(String, String)> {
    sqlx::query_as("SELECT key, params FROM notifications ORDER BY id")
        .fetch_all(db)
        .await
        .expect("notifications")
}

#[tokio::test]
async fn saves_in_a_row_leave_one_notice_for_each_section() {
    let db = database::open_in_memory().await.expect("database");
    for _ in 0..3 {
        record_saved(&db, Section::General).await;
    }
    record_saved(&db, Section::Power).await;
    assert_eq!(notices(&db).await.len(), 2);

    sqlx::query("UPDATE notifications SET read = 1")
        .execute(&db)
        .await
        .expect("read");
    record_saved(&db, Section::General).await;
    let listed = notices(&db).await;
    assert_eq!(listed.len(), 3);
    assert_eq!(listed[2].0, "settingsSaved");
    assert!(listed[2].1.contains("general"));
}

#[tokio::test]
async fn an_old_notice_is_not_refreshed() {
    let db = database::open_in_memory().await.expect("database");
    record_saved(&db, Section::General).await;
    sqlx::query("UPDATE notifications SET created_at = created_at - 60000")
        .execute(&db)
        .await
        .expect("older");
    record_saved(&db, Section::General).await;
    assert_eq!(notices(&db).await.len(), 2);
}
