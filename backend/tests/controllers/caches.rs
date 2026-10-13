use super::*;
use crate::services::database;
use axum::{body::Body, http::Request};
use tower::ServiceExt;

#[tokio::test]
async fn the_caches_are_listed_and_a_wrong_one_is_refused() {
    let db = database::open_in_memory().await.expect("database");
    let (router, _) = routes().split_for_parts();
    let router = router.with_state(AppState::new(db));
    let listed = router
        .clone()
        .oneshot(
            Request::get("/caches")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("answer");
    assert_eq!(listed.status(), StatusCode::OK);
    let refused = router
        .oneshot(
            Request::delete("/caches/nas-not-installed/previews")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("answer");
    assert_eq!(refused.status(), StatusCode::NOT_FOUND);
}
