use super::*;
use axum::http::StatusCode;

#[tokio::test]
async fn only_an_application_that_declares_it_may_stay_in_background() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("in-memory database");

    let refused = declare(&db, "not-installed", true)
        .await
        .expect_err("refused");
    assert_eq!(refused.0, StatusCode::FORBIDDEN);
    assert!(refused.1.contains("run.background"), "{}", refused.1);
    assert!(!keeps_running(&db, "not-installed").await);

    start(db).await;
}
