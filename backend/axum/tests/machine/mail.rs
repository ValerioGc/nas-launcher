use crate::support::Machine;
use axum::http::StatusCode;
use nas_os_axum::services::{
    database, mail_events,
    notifications::Notice,
    session::{Session, SessionData},
};
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;

#[tokio::test]
async fn sends_the_codes_of_the_account_through_the_email_application() {
    Machine::get();

    let status = mail_events::status().await;
    assert!(status.installed && status.sender);
    mail_events::send_code("owner@example.com", "verify", "123456")
        .await
        .expect("sent");
    let refused = mail_events::send_code("refused@example.com", "reset", "1").await;
    assert_eq!(refused.unwrap_err().0, StatusCode::BAD_REQUEST);
    let broken = mail_events::send_code("broken@example.com", "reset", "1").await;
    assert_eq!(broken.unwrap_err().0, StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn forwards_events_and_watches_for_updates_while_it_is_installed() {
    Machine::get();
    let db = database::open_in_memory().await.expect("database");

    mail_events::forward(&Notice::new("nasUpdated", "NAS updated", "Updated."));
    let watching = tokio::spawn(mail_events::watch(
        db.clone(),
        Session(Arc::new(RwLock::new(SessionData::default()))),
    ));
    for _ in 0..100 {
        let announced: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM notifications WHERE key = 'systemUpdatesAvailable'",
        )
        .fetch_one(&db)
        .await
        .expect("notifications");
        if announced > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    watching.abort();
    let announced: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE key = 'systemUpdatesAvailable'",
    )
    .fetch_one(&db)
    .await
    .expect("notifications");
    assert_eq!(announced, 1);
}
