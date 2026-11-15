use super::access_guard;
use crate::{http::state::AppState, services::database};
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
    middleware,
    routing::get,
    Router,
};
use sqlx::SqlitePool;
use std::net::SocketAddr;
use tower::ServiceExt;

fn guarded(state: &AppState) -> Router {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(state.clone(), access_guard))
        .with_state(state.clone())
}

async fn status(router: &Router, peer: &str) -> StatusCode {
    let mut request = Request::builder()
        .uri("/")
        .body(Body::empty())
        .expect("request");
    let peer: SocketAddr = peer.parse().expect("peer address");
    request.extensions_mut().insert(ConnectInfo(peer));
    router
        .clone()
        .oneshot(request)
        .await
        .expect("answer")
        .status()
}

async fn allow(db: &SqlitePool, value: &str) {
    sqlx::query("INSERT INTO access_rules (kind, value, created_at) VALUES ('ip', ?, 0)")
        .bind(value)
        .execute(db)
        .await
        .expect("rule");
}

async fn state() -> AppState {
    AppState::new(database::open_in_memory().await.expect("database"))
}

#[tokio::test]
async fn an_empty_list_lets_every_address_in() {
    let state = state().await;
    let router = guarded(&state);

    assert_eq!(status(&router, "192.168.1.20:4000").await, StatusCode::OK);
}

#[tokio::test]
async fn only_the_listed_addresses_get_in_and_the_machine_itself_always_does() {
    let state = state().await;
    allow(&state.db, "192.168.1.10").await;
    let router = guarded(&state);

    assert_eq!(status(&router, "192.168.1.10:4000").await, StatusCode::OK);
    assert_eq!(
        status(&router, "192.168.1.20:4000").await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(status(&router, "127.0.0.1:4000").await, StatusCode::OK);
}

#[tokio::test]
async fn the_rules_are_read_once_and_again_only_after_a_change() {
    let state = state().await;
    allow(&state.db, "192.168.1.10").await;
    let router = guarded(&state);
    assert_eq!(
        status(&router, "192.168.1.20:4000").await,
        StatusCode::FORBIDDEN
    );

    allow(&state.db, "192.168.1.20").await;
    assert_eq!(
        status(&router, "192.168.1.20:4000").await,
        StatusCode::FORBIDDEN,
        "the list in memory until a change says so"
    );

    state.access.changed();
    assert_eq!(status(&router, "192.168.1.20:4000").await, StatusCode::OK);
}
