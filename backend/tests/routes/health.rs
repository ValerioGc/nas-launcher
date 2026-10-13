use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

use nas_os_axum::{build_router, services::database, AppState};

fn use_fixture_catalogue() {
    unsafe {
        std::env::set_var(
            "NAS_CATALOGUE_PATH",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/catalogue.json"
            ),
        );
    }
}

async fn get(path: &str) -> (StatusCode, serde_json::Value) {
    call(
        Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("valid request"),
    )
    .await
}

async fn post(path: &str) -> (StatusCode, serde_json::Value) {
    call(
        Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from("{}"))
            .expect("valid request"),
    )
    .await
}

fn use_empty_apps_directory() {
    unsafe {
        std::env::set_var(
            "NAS_APPS_DIR",
            concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/no-apps"),
        );
    }
}

async fn call(request: Request<Body>) -> (StatusCode, serde_json::Value) {
    use_empty_apps_directory();
    let db = database::open_in_memory()
        .await
        .expect("in-memory database");
    let response = build_router(AppState::new(db))
        .oneshot(request)
        .await
        .expect("router response");

    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();

    (status, serde_json::from_slice(&body).expect("JSON body"))
}

#[tokio::test]
async fn health_reports_the_service_status_and_version() {
    let (status, body) = get("/health").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn unknown_routes_answer_with_a_json_error() {
    let (status, body) = get("/missing").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "resource not found");
}

#[tokio::test]
async fn status_is_available_under_the_api_prefix() {
    let (status, body) = get("/api/status").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["online"], true);
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn the_installed_applications_are_listed_for_the_shell() {
    let (status, body) = get("/api/apps").await;

    assert_eq!(status, StatusCode::OK);

    let apps = body.as_array().expect("an array of applications");
    assert_eq!(apps.len(), 5);
    assert_eq!(apps[0]["id"], "nas-app-manager");
    assert_eq!(apps[0]["system"], true);
    assert_eq!(apps[0]["running"], true);
    assert!(apps[0]["name"].is_string());
    assert!(apps[0]["info"].is_string());
}

#[tokio::test]
async fn an_unknown_application_answers_with_a_typed_error() {
    let (status, body) = get("/api/apps/not-an-application/info").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("not-an-application"));
}

#[tokio::test]
async fn the_available_applications_come_from_the_catalogue_file() {
    use_fixture_catalogue();
    let (status, body) = get("/api/apps/available").await;

    assert_eq!(status, StatusCode::OK);
    let offered = body.as_array().expect("an array");
    assert_eq!(offered.len(), 2);
    assert_eq!(offered[0]["id"], "fileExplorer");
    assert_eq!(offered[0]["repository"], "example/nas-file-explorer");
    // Nothing is installed on an empty database, so nothing has a version yet.
    assert!(offered[0]["installedVersion"].is_null());
    assert_eq!(offered[0]["updateAvailable"], false);
}

#[tokio::test]
async fn checking_for_updates_is_refused_where_packages_are_disabled() {
    use_fixture_catalogue();
    let (status, body) = post("/api/apps/updates").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("Package operations are not available"));
}

#[tokio::test]
async fn installing_is_refused_the_same_way() {
    use_fixture_catalogue();
    let (status, _) = post("/api/apps/fileExplorer/install").await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn the_forge_token_is_never_handed_back() {
    let (status, body) = get("/api/apps/credentials").await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["configured"].is_boolean());
    assert!(body.get("token").is_none());
    assert_eq!(body["packagesEnabled"], false);
    assert_eq!(body["appsManaged"], false);
}

#[tokio::test]
async fn every_answer_asks_search_engines_not_to_index_it() {
    let db = database::open_in_memory()
        .await
        .expect("in-memory database");
    let request = Request::builder()
        .uri("/api/unknown")
        .body(Body::empty())
        .expect("valid request");
    let response = build_router(AppState::new(db))
        .oneshot(request)
        .await
        .expect("router response");

    assert_eq!(
        response.headers()["x-robots-tag"],
        "noindex, nofollow, noarchive, nosnippet, noimageindex"
    );
}

#[tokio::test]
async fn only_the_launcher_pages_may_change_something() {
    use_fixture_catalogue();
    for (site, expected) in [
        ("cross-site", StatusCode::FORBIDDEN),
        ("same-site", StatusCode::FORBIDDEN),
        ("same-origin", StatusCode::SERVICE_UNAVAILABLE),
    ] {
        let (status, _) = call(
            Request::builder()
                .method("POST")
                .uri("/api/apps/updates")
                .header("content-type", "application/json")
                .header("sec-fetch-site", site)
                .body(Body::from("{}"))
                .expect("valid request"),
        )
        .await;
        assert_eq!(status, expected, "{site}");
    }
}

#[tokio::test]
async fn the_setup_is_open_on_a_new_nas_and_closed_by_the_wizard() {
    let (status, body) = get("/api/setup").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["completed"], false);

    let (status, body) = call(
        Request::builder()
            .method("POST")
            .uri("/api/setup/complete")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"language":"it","theme":"system"}"#))
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["completed"], true);

    let (status, _) = post("/api/setup/complete").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_wizard_hands_over_language_and_theme_only() {
    let (status, _) = call(
        Request::builder()
            .method("POST")
            .uri("/api/setup/complete")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"username":"admin","language":"it","theme":"system"}"#,
            ))
            .expect("valid request"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = post("/api/setup/network/scan").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_token_that_is_not_one_is_refused() {
    let (status, body) = call(
        Request::builder()
            .method("PUT")
            .uri("/api/apps/credentials")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"token":"two words"}"#))
            .expect("valid request"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "that does not look like a token");
}

#[tokio::test]
async fn no_shell_and_no_log_where_the_terminal_is_disabled() {
    for path in ["/api/terminal/session", "/api/terminal/log"] {
        let (status, body) = get(path).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{path}");
        assert!(body["error"]
            .as_str()
            .expect("a reason")
            .contains("The terminal is not available"));
    }
}

#[tokio::test]
async fn a_command_goes_to_an_open_session_and_on_one_line_only() {
    let router = session().await;

    let (status, _) = send(
        &router,
        "POST",
        "/api/terminal/session/nothing-open/input",
        r#"{"command":"ls"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, body) = send(
        &router,
        "POST",
        "/api/terminal/session/nothing-open/input",
        r#"{"command":"ls\nrm -rf /tmp/x"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid command");
}

#[tokio::test]
async fn terminal_settings_out_of_range_are_refused() {
    let (status, body) = call(
        Request::builder()
            .method("PUT")
            .uri("/api/settings/terminal")
            .header("content-type", "application/json")
            .body(Body::from(
                r##"{"transparency":0,"textColor":"#ffffff","backgroundColor":"#000000","highlightColor":"#3fd39b","fontSize":80,"lineHeight":1.5}"##,
            ))
            .expect("valid request"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid terminal settings");
}

async fn session() -> axum::Router {
    use_empty_apps_directory();
    use_fixture_catalogue();
    let db = database::open_in_memory()
        .await
        .expect("in-memory database");
    build_router(AppState::new(db))
}

async fn send(
    router: &axum::Router,
    method: &str,
    path: &str,
    body: &str,
) -> (StatusCode, serde_json::Value) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_owned()))
                .expect("valid request"),
        )
        .await
        .expect("router response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    (status, serde_json::from_slice(&bytes).expect("JSON body"))
}

#[tokio::test]
async fn an_edited_catalogue_is_what_the_launcher_offers() {
    let router = session().await;
    let (_, before) = send(&router, "GET", "/api/apps/catalogue", "").await;
    assert_eq!(before.as_array().expect("entries").len(), 2);

    let (status, saved) = send(
        &router,
        "PUT",
        "/api/apps/catalogue",
        r#"{"apps":[{"id":"photos","name":"Photos","provider":"github","repository":"example/photos","asset":"photos-{version}.tar.gz","logo":"https://example.com/photos.svg"}]}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved[0]["id"], "photos");

    let (_, offered) = send(&router, "GET", "/api/apps/available", "").await;
    assert_eq!(offered.as_array().expect("offers").len(), 1);
    assert_eq!(offered[0]["name"], "Photos");
    assert_eq!(offered[0]["logo"], "");
    assert!(saved[0].get("logo").is_some_and(|logo| logo == ""));
}

#[tokio::test]
async fn the_logo_of_a_catalogue_entry_is_uploaded_and_served_by_the_launcher() {
    let router = session().await;
    send(
        &router,
        "PUT",
        "/api/apps/catalogue",
        r#"{"apps":[{"id":"photos","name":"Photos","provider":"github","repository":"example/photos","asset":"photos-{version}.tar.gz"}]}"#,
    )
    .await;
    let raw = |method: &str, path: &str, body: &'static [u8]| {
        router.clone().oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "image/svg+xml")
                .body(Body::from(body))
                .expect("valid request"),
        )
    };
    let svg: &'static [u8] = br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#;

    let refused = raw("PUT", "/api/apps/photos/logo", b"<html/>")
        .await
        .expect("response");
    assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
    let stored = raw("PUT", "/api/apps/photos/logo", svg)
        .await
        .expect("response");
    assert_eq!(stored.status(), StatusCode::NO_CONTENT);

    let served = raw("GET", "/api/apps/photos/logo", b"")
        .await
        .expect("response");
    assert_eq!(served.status(), StatusCode::OK);
    assert_eq!(served.headers()["content-type"], "image/svg+xml");
    assert!(served.headers()["content-security-policy"]
        .to_str()
        .expect("policy")
        .contains("sandbox"));
    let (_, catalogue) = send(&router, "GET", "/api/apps/catalogue", "").await;
    assert!(catalogue[0]["logo"]
        .as_str()
        .is_some_and(|logo| logo.starts_with("/api/apps/photos/logo?v=")));

    let large: &'static [u8] =
        Box::leak([svg, &vec![b' '; 200 * 1024]].concat().into_boxed_slice());
    let stored = raw("PUT", "/api/apps/photos/logo", large)
        .await
        .expect("response");
    assert_eq!(stored.status(), StatusCode::NO_CONTENT);

    let removed = raw("DELETE", "/api/apps/photos/logo", b"")
        .await
        .expect("response");
    assert_eq!(removed.status(), StatusCode::NO_CONTENT);
    let (status, _) = send(&router, "GET", "/api/apps/photos/logo", "").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_catalogue_entry_that_is_not_valid_leaves_the_catalogue_as_it_was() {
    let router = session().await;

    let (status, body) = send(
        &router,
        "PUT",
        "/api/apps/catalogue",
        r#"{"apps":[{"id":"../escape","name":"X","provider":"github","repository":"a/b","asset":"x.tar.gz"}]}"#,
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "invalid catalogue entry");
    let (_, after) = send(&router, "GET", "/api/apps/catalogue", "").await;
    assert_eq!(after.as_array().expect("entries").len(), 2);
}

#[tokio::test]
async fn the_automatic_update_check_is_a_settings_section() {
    let router = session().await;
    let (_, defaults) = send(&router, "GET", "/api/settings/apps", "").await;
    assert_eq!(defaults["autoCheck"], false);
    assert_eq!(defaults["frequency"], "daily");
    assert_eq!(defaults["checkOnOpen"], false);
    assert_eq!(defaults["updateOnOpen"], false);

    let (status, _) = send(
        &router,
        "PUT",
        "/api/settings/apps",
        r#"{"autoCheck":true,"frequency":"hourly"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, saved) = send(&router, "GET", "/api/settings/apps", "").await;
    assert_eq!(saved["autoCheck"], true);
    assert_eq!(saved["frequency"], "hourly");

    let (status, _) = send(
        &router,
        "PUT",
        "/api/settings/apps",
        r#"{"autoCheck":true,"frequency":"minutely"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = send(
        &router,
        "PUT",
        "/api/settings/apps",
        r#"{"autoCheck":false,"frequency":"daily","checkOnOpen":false,"updateOnOpen":true}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_request_for_an_application_that_is_not_installed_says_so() {
    let router = session().await;
    let call = |path: &'static str| {
        router.clone().oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("valid request"),
        )
    };

    let missing = call("/apps/nothing/index.html").await.expect("response");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let bare = call("/apps/nothing").await.expect("response");
    assert_eq!(bare.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(bare.headers()["location"], "/apps/nothing/");
    let (status, _) = send(&router, "GET", "/api/apps", "").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn the_log_of_a_notification_is_read_under_var_log_only() {
    let router = session().await;
    let (status, _) = send(&router, "GET", "/api/notifications/999/log", "").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, created) = send(
        &router,
        "POST",
        "/api/notifications",
        r#"{"title":"Backup failed","message":"","source":"backup","tone":"error","log":"/etc/passwd"}"#,
    )
    .await;
    let path = format!(
        "/api/notifications/{}/log",
        created["id"].as_str().expect("id")
    );
    let (status, body) = send(&router, "GET", &path, "").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}
