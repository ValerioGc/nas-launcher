use super::*;
use crate::services::database;
use axum::{body::Body, http::Request};
use tower::ServiceExt;

/// The routes as the application `nas-backup-manager` reaches them.
async fn as_backup() -> (Router, SqlitePool) {
    let db = database::open_in_memory().await.expect("database");
    let router = routes()
        .layer(Extension(Caller("nas-backup-manager".to_owned())))
        .with_state(AppState::new(db.clone()));
    (router, db)
}

#[tokio::test]
async fn only_the_terminal_application_reaches_its_journal_and_settings() {
    let (other, db) = as_backup().await;
    for path in ["/api/settings/terminal", "/api/terminal/log"] {
        assert_eq!(send(&other, "GET", path, "").await, StatusCode::FORBIDDEN);
    }
    let terminal = routes()
        .layer(Extension(Caller("nas-system-terminal".to_owned())))
        .with_state(AppState::new(db));
    assert_eq!(
        send(&terminal, "GET", "/api/settings/terminal", "").await,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &terminal,
            "POST",
            "/api/terminal/session/missing/input",
            r#"{"command":"ls"}"#
        )
        .await,
        StatusCode::NOT_FOUND
    );
}

async fn send(router: &Router, method: &str, path: &str, body: &str) -> StatusCode {
    router
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_owned()))
                .expect("request"),
        )
        .await
        .expect("answer")
        .status()
}

fn task(id: &str, source: &str) -> String {
    format!(r#"{{"id":"{id}","source":"{source}","name":"Copia"}}"#)
}

#[tokio::test]
async fn an_application_notifies_only_under_its_own_name() {
    let (router, _) = as_backup().await;
    let notice = |source: &str| {
        format!(
            r#"{{"title":"Copia completata","message":"","source":"{source}","tone":"success"}}"#
        )
    };

    assert_eq!(
        send(
            &router,
            "POST",
            "/api/notifications",
            &notice("nas-backup-manager")
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &router,
            "POST",
            "/api/notifications",
            &notice("nas-system-settings")
        )
        .await,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn an_application_runs_its_own_operations_and_leaves_the_others_alone() {
    let (router, db) = as_backup().await;
    tasks::register(
        &db,
        NewTask {
            id: "install-photos".to_owned(),
            source: "nas-app-manager".to_owned(),
            name: "Installazione".to_owned(),
            detail: String::new(),
            progress: None,
            pausable: false,
            cancellable: false,
        },
    )
    .await
    .expect("registered");

    let own = task("nas-backup-manager-1", "nas-backup-manager");
    assert_eq!(
        send(&router, "POST", "/api/tasks", &own).await,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &router,
            "PATCH",
            "/api/tasks/nas-backup-manager-1",
            r#"{"progress":40}"#
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(send(&router, "GET", "/api/tasks", "").await, StatusCode::OK);

    let impostor = task("install-photos", "nas-app-manager");
    assert_eq!(
        send(&router, "POST", "/api/tasks", &impostor).await,
        StatusCode::FORBIDDEN
    );
    let takeover = task("install-photos", "nas-backup-manager");
    assert_eq!(
        send(&router, "POST", "/api/tasks", &takeover).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &router,
            "PATCH",
            "/api/tasks/install-photos",
            r#"{"state":"cancelled"}"#
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(&router, "DELETE", "/api/tasks/install-photos", "").await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        tasks::get(&db, "install-photos")
            .await
            .expect("kept")
            .source,
        "nas-app-manager"
    );

    assert_eq!(
        send(&router, "DELETE", "/api/tasks/nas-backup-manager-1", "").await,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn an_application_reads_the_limit_of_inotify_it_cannot_read_itself() {
    let (router, _) = as_backup().await;
    let answer = router
        .oneshot(
            Request::builder()
                .uri("/api/system/inotify")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("answer");
    assert_eq!(answer.status(), StatusCode::OK);
    let body = axum::body::to_bytes(answer.into_body(), 1024)
        .await
        .expect("body");
    let limit: u64 = std::fs::read_to_string(INOTIFY_WATCHES)
        .expect("limit")
        .trim()
        .parse()
        .expect("number");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).expect("json"),
        serde_json::json!({ "maxUserWatches": limit })
    );
}

#[tokio::test]
async fn only_an_application_that_asks_for_it_cleans_the_system_or_the_caches() {
    let (router, _) = as_backup().await;
    for (method, path, body) in [
        ("POST", "/api/apps/self/cleanup", r#"{"action":"overview"}"#),
        ("GET", "/api/caches", ""),
        ("DELETE", "/api/caches/nas-file-manager/previews", ""),
    ] {
        assert_eq!(
            send(&router, method, path, body).await,
            StatusCode::FORBIDDEN,
            "{method} {path}"
        );
    }
}

#[tokio::test]
async fn nothing_else_of_the_launcher_is_on_this_socket() {
    let (router, _) = as_backup().await;
    for (method, path) in [
        ("POST", "/api/power/restart"),
        ("PUT", "/api/settings/general"),
        ("POST", "/api/apps/nas-file-manager/uninstall"),
        ("DELETE", "/api/notifications/1"),
        ("POST", "/api/tasks/finished"),
        ("GET", "/api/desktop/shortcuts"),
    ] {
        let status = send(&router, method, path, "{}").await;
        assert!(
            status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED,
            "{method} {path}: {status}"
        );
    }
}

#[tokio::test]
async fn an_application_says_only_of_itself_whether_it_works_in_background() {
    let (router, _) = as_backup().await;
    // Not installed here, so its manifest declares nothing: refused, and the body is checked.
    assert_eq!(
        send(
            &router,
            "PUT",
            "/api/apps/self/background",
            r#"{"needed":true}"#
        )
        .await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &router,
            "PUT",
            "/api/apps/self/background",
            r#"{"needed":"yes"}"#
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(&router, "GET", "/api/apps/self/background", "").await,
        StatusCode::METHOD_NOT_ALLOWED
    );
}

#[tokio::test]
async fn an_application_shares_only_what_its_manifest_allows() {
    let (router, _) = as_backup().await;
    let shares = r#"{"shares":[{"name":"Foto","path":"/srv/storage/d/Foto","writable":false,"protected":true}]}"#;
    // Not installed here: its manifest declares nothing, and the body is checked first.
    assert_eq!(
        send(&router, "PUT", "/api/apps/self/shares", shares).await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        send(
            &router,
            "PUT",
            "/api/apps/self/shares",
            r#"{"shares":"all"}"#
        )
        .await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        send(&router, "GET", "/api/apps/self/shares", "").await,
        StatusCode::OK
    );
}
