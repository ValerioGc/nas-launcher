use crate::support::Machine;
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use nas_os_axum::{
    build_router, http::extract::REAUTHENTICATED_HEADER as REAUTHENTICATED, services::database,
    AppState,
};
use serde_json::{json, Value};
use tower::ServiceExt;

struct Api(axum::Router);

impl Api {
    async fn new() -> Self {
        Machine::get();
        let db = database::open_in_memory().await.expect("database");
        Self(build_router(AppState::new(db)))
    }

    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        self.call_as(None, method, path, body).await
    }

    async fn call_as(
        &self,
        user: Option<&str>,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let body = body.or_else(|| (method != Method::GET).then(|| json!({})));
        let mut request = Request::builder().method(method.clone()).uri(path);
        if body.is_some() {
            request = request.header("content-type", "application/json");
        }
        if let Some(user) = user {
            request = request.header(REAUTHENTICATED, user);
        }
        let request = request
            .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
            .expect("request");
        let response = self.0.clone().oneshot(request).await.expect("response");
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        if std::env::var_os("NAS_ROUTES_DEBUG").is_some() {
            eprintln!("{method} {path} -> {status}");
        }
        assert!(
            !status.is_server_error() || status == StatusCode::SERVICE_UNAVAILABLE,
            "{method} {path} answered {status}: {value}"
        );
        (status, value)
    }

    async fn get(&self, path: &str) -> Value {
        let (status, value) = self.call(Method::GET, path, None).await;
        assert!(status.is_success(), "GET {path} answered {status}: {value}");
        value
    }
}

#[tokio::test]
async fn every_setting_is_read_and_saved_back_as_it_was() {
    let api = Api::new().await;
    for path in [
        "/api/settings/general",
        "/api/settings/access",
        "/api/settings/power",
        "/api/settings/notifications",
        "/api/settings/terminal",
        "/api/settings/widgets",
        "/api/settings/apps",
    ] {
        let read = api.get(path).await;
        let (status, saved) = api.call(Method::PUT, path, Some(read.clone())).await;
        assert!(status.is_success(), "PUT {path} answered {status}: {saved}");
    }
    let listed = api.get("/api/apps/catalogue").await;
    let apps = listed
        .as_array()
        .expect("listed apps")
        .iter()
        .map(|app| {
            let field = |name: &str| app[name].clone();
            json!({
                "id": field("id"), "name": field("name"), "info": field("info"),
                "provider": field("provider"), "repository": field("repository"), "asset": field("asset"),
            })
        })
        .collect::<Vec<_>>();
    let (status, saved) = api
        .call(
            Method::PUT,
            "/api/apps/catalogue",
            Some(json!({ "apps": apps })),
        )
        .await;
    assert!(status.is_success(), "{saved}");
}

#[tokio::test]
async fn reads_the_state_of_the_machine_and_of_its_applications() {
    let api = Api::new().await;
    for path in [
        "/api/status",
        "/api/mail/status",
        "/api/system/telemetry",
        "/api/setup",
        "/api/storage/disks/external",
        "/api/storage/disks?kind=local",
        "/api/storage/encryption",
        "/api/storage/network",
        "/api/storage/locks",
        "/api/apps",
        "/api/apps/installed",
        "/api/apps/available",
        "/api/apps/credentials",
        "/api/apps/nas-app-manager/info",
        "/api/system/power",
        "/api/system/power/busy",
        "/api/system/config",
        "/api/system/config/apps",
        "/api/system/reset",
        "/api/system/updates",
        "/api/launcher/updates",
        "/api/notifications",
        "/api/notifications/sources",
        "/api/terminal/log",
        "/api/tasks",
        "/api/access/rules",
        "/api/capabilities/file-picker",
        "/api/network",
    ] {
        api.call(Method::GET, path, None).await;
    }
    for (method, path) in [
        (Method::GET, "/api/apps/probe/icon"),
        (Method::GET, "/api/apps/probe/logo"),
        (Method::DELETE, "/api/apps/probe/logo"),
        (Method::POST, "/api/apps/updates"),
        (Method::POST, "/api/apps/nowhere/start"),
        (Method::POST, "/api/system/updates/refresh"),
        (Method::POST, "/api/launcher/updates/check"),
        (Method::POST, "/api/launcher/updates"),
        (Method::POST, "/api/network/scan"),
        (Method::POST, "/api/system/actions/restart"),
        (Method::GET, "/api/system/power"),
        (Method::POST, "/api/system/actions/explode"),
    ] {
        api.call(method, path, None).await;
    }
}

#[tokio::test]
async fn notifications_tasks_and_shortcuts_come_and_go() {
    let api = Api::new().await;

    let (status, created) = api
        .call(
            Method::POST,
            "/api/notifications",
            Some(json!({"title": "Backup", "message": "Done", "source": "nas-backup-manager", "tone": "success"})),
        )
        .await;
    assert!(status.is_success(), "{created}");
    let id = created["id"].as_str().expect("id").to_owned();
    api.call(
        Method::PATCH,
        &format!("/api/notifications/{id}"),
        Some(json!({"read": true})),
    )
    .await;
    api.call(Method::GET, &format!("/api/notifications/{id}/log"), None)
        .await;
    api.call(Method::POST, "/api/notifications/read", None)
        .await;
    api.call(Method::POST, "/api/notifications/read/clear", None)
        .await;
    api.call(Method::DELETE, &format!("/api/notifications/{id}"), None)
        .await;

    let task = json!({"id": "copy-1", "source": "nas-file-manager", "name": "Copy", "pausable": true, "cancellable": true});
    let (status, _) = api.call(Method::POST, "/api/tasks", Some(task)).await;
    assert!(status.is_success());
    api.call(
        Method::PATCH,
        "/api/tasks/copy-1",
        Some(json!({"progress": 50})),
    )
    .await;
    api.call(Method::POST, "/api/tasks/copy-1/pause", None)
        .await;
    api.call(Method::POST, "/api/tasks/copy-1/resume", None)
        .await;
    api.call(Method::POST, "/api/tasks/copy-1/cancel", None)
        .await;
    api.call(Method::POST, "/api/tasks/finished", None).await;
    api.call(Method::DELETE, "/api/tasks/copy-1", None).await;

    api.call(Method::POST, "/api/desktop/shortcuts/nas-app-manager", None)
        .await;
    api.call(
        Method::POST,
        "/api/desktop/folders",
        Some(json!({"path": "/Photos", "name": "Photos"})),
    )
    .await;
    let shortcuts = api.get("/api/desktop/shortcuts").await;
    let order = shortcuts
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| item["key"].clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    api.call(
        Method::PUT,
        "/api/desktop/shortcuts/order",
        Some(json!({"order": order})),
    )
    .await;
    api.call(
        Method::DELETE,
        "/api/desktop/folders",
        Some(json!({"path": "/Photos"})),
    )
    .await;
    api.call(
        Method::DELETE,
        "/api/desktop/shortcuts/nas-app-manager",
        None,
    )
    .await;
}

#[tokio::test]
async fn access_rules_failures_locks_and_the_token_are_checked() {
    let api = Api::new().await;
    let (status, rules) = api
        .call(
            Method::POST,
            "/api/access/rules",
            Some(json!({"kind": "ip", "value": "10.0.0.7"})),
        )
        .await;
    assert!(status.is_success(), "{rules}");
    let id = rules[0]["id"].as_str().unwrap_or("0").to_owned();
    api.call(Method::DELETE, &format!("/api/access/rules/{id}"), None)
        .await;
    api.call(
        Method::POST,
        "/api/access/rules",
        Some(json!({"kind": "ip", "value": "nope"})),
    )
    .await;
    api.call(
        Method::POST,
        "/api/access/failures",
        Some(json!({"field": "password"})),
    )
    .await;
    api.call(
        Method::POST,
        "/api/storage/locks",
        Some(json!({"uuid": "1234-abcd", "locked": true})),
    )
    .await;
    api.call(
        Method::PUT,
        "/api/apps/credentials",
        Some(json!({"token": "ghp_example"})),
    )
    .await;
    for (path, body) in [
        (
            "/api/storage/disks/initialize",
            json!({"device": "sdz", "label": "Data"}),
        ),
        (
            "/api/storage/disks/rename",
            json!({"device": "sdz1", "label": "Data"}),
        ),
        ("/api/storage/disks/smart", json!({"device": "sdz"})),
        ("/api/storage/disks/eject", json!({"device": "sdz"})),
        (
            "/api/storage/encryption",
            json!({"op": "lock", "uuid": "not a uuid"}),
        ),
        (
            "/api/storage/network",
            json!({"op": "networkConnect", "id": "not an id"}),
        ),
        (
            "/api/network/wifi",
            json!({"ssid": "home", "password": "long enough"}),
        ),
        (
            "/api/system/processes/1",
            json!({"action": "terminate", "started": 1}),
        ),
        (
            "/api/setup/complete",
            json!({"language": "it", "theme": "dark", "nasName": "attic"}),
        ),
    ] {
        api.call(Method::POST, path, Some(body)).await;
    }
}

#[tokio::test]
async fn opens_its_socket_to_the_applications_and_turns_anybody_else_away() {
    use nas_os_axum::http::app_channel;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    Machine::get();
    let db = database::open_in_memory().await.expect("database");
    let path = app_channel::socket_path();
    let server = tokio::spawn(app_channel::serve(AppState::new(db)));
    for _ in 0..200 {
        if path.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let mut stream = tokio::net::UnixStream::connect(&path)
        .await
        .expect("connected");
    stream
        .write_all(b"GET /api/notifications HTTP/1.1\r\nHost: nas\r\nConnection: close\r\n\r\n")
        .await
        .expect("request");
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.expect("answer");
    server.abort();
    assert!(answer.starts_with("HTTP/1.1 403"), "{answer}");
}

#[tokio::test]
async fn exports_and_imports_the_configuration_once_the_password_is_confirmed() {
    let api = Api::new().await;
    let (refused, _) = api.call(Method::GET, "/api/system/config", None).await;
    assert_eq!(refused, StatusCode::FORBIDDEN);

    let (status, exported) = api
        .call_as(
            Some("owner"),
            Method::GET,
            "/api/system/config?apps=nas-notification-manager,nas-system-terminal",
            None,
        )
        .await;
    assert!(status.is_success(), "{exported}");
    let (status, imported) = api
        .call_as(
            Some("owner"),
            Method::POST,
            "/api/system/config",
            Some(exported),
        )
        .await;
    assert!(status.is_success(), "{imported}");
    let (status, _) = api
        .call_as(
            Some("owner"),
            Method::POST,
            "/api/system/config",
            Some(json!({"version": 99})),
        )
        .await;
    assert!(status.is_client_error());
}
