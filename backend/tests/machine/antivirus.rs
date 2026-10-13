use crate::support::Machine;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use nas_os_axum::{
    apps::services::antivirus::{self, LifecycleNotice, Report, ScanKind},
    build_router,
    services::{
        database,
        settings::{self, Section},
    },
    AppState,
};
use serde_json::{json, Value};
use tower::ServiceExt;
async fn call(
    router: &axum::Router,
    method: &str,
    path: &str,
    input: Option<Value>,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(input.map_or(Body::empty(), |input| Body::from(input.to_string())))
        .expect("request");
    let response = router.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn idle(db: &sqlx::SqlitePool) {
    for _ in 0..200 {
        if antivirus::status(db)
            .await
            .expect("status")
            .operation
            .phase
            .is_empty()
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("operation did not complete");
}
#[tokio::test]
async fn antivirus_routes_install_configure_scan_notify_and_recover_without_native_machine_changes()
{
    let machine = Machine::get();
    let db = database::open_in_memory().await.expect("database");
    let router = build_router(AppState::new(db.clone()));
    let (_, defaults) = call(&router, "GET", "/api/antivirus", None).await;
    assert_eq!(defaults["settings"]["enabled"], false);
    assert_eq!(defaults["settings"]["bootScan"]["enabled"], false);
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/antivirus/enabled",
            Some(json!({"enabled":true}))
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    // A second operation cannot overwrite the request of the first one.
    assert!(antivirus::toggle(&db, true).await.is_err());
    idle(&db).await;
    assert!(antivirus::load(&db).await.expect("enabled").enabled);
    let mut config = antivirus::load(&db).await.expect("settings");
    config.boot_scan.enabled = true;
    config.boot_scan.kind = ScanKind::Full;
    config.shutdown_scan.enabled = true;
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/antivirus/settings",
            Some(serde_json::to_value(&config).expect("settings"))
        )
        .await
        .0,
        StatusCode::OK
    );
    for found in [0, 2] {
        let report = Report {
            found,
            scanned: 10,
            ..Report::default()
        };
        std::fs::write(
            database::beside("antivirus.result.json").expect("result"),
            serde_json::to_vec(&report).expect("report"),
        )
        .expect("result file");
        assert_eq!(
            call(
                &router,
                "POST",
                "/api/antivirus/scan",
                Some(json!({"kind":"quick","paths":[]}))
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
        idle(&db).await;
    }
    for action in ["quarantine", "restore", "delete"] {
        assert_eq!(
            call(
                &router,
                "POST",
                &format!("/api/antivirus/quarantine/{action}"),
                Some(json!({"id":"one"}))
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
        idle(&db).await;
    }
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/antivirus/quarantine/invalid",
            Some(json!({"id":"one"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/antivirus/scan",
            Some(json!({"kind":"file","paths":[]}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let fail = machine.root.join("fail/systemctl@nas-os-antivirus.service");
    std::fs::write(&fail, "native helper failed").expect("failure");
    let report = Report {
        updated_at: i64::MAX,
        operation_error: Some("ClamAV database error".into()),
        ..Report::default()
    };
    std::fs::write(
        database::beside("antivirus.result.json").expect("result"),
        serde_json::to_vec(&report).expect("report"),
    )
    .expect("result file");
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/antivirus/scan",
            Some(json!({"kind":"quick","paths":[]}))
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    idle(&db).await;
    assert_eq!(
        antivirus::status(&db)
            .await
            .expect("status")
            .operation
            .error
            .as_deref(),
        Some("ClamAV database error")
    );
    assert_eq!(
        call(
            &router,
            "PUT",
            "/api/antivirus/settings",
            Some(serde_json::to_value(&config).expect("settings"))
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    std::fs::remove_file(&fail).expect("remove failure");
    antivirus::toggle(&db, false).await.expect("disable");
    idle(&db).await;
    assert!(!antivirus::load(&db).await.expect("disabled").enabled);
    std::fs::write(&fail, "enable helper failed").expect("failure");
    antivirus::toggle(&db, true).await.expect("start install");
    idle(&db).await;
    assert!(
        !antivirus::load(&db)
            .await
            .expect("failed enable stays disabled")
            .enabled
    );
    std::fs::remove_file(&fail).expect("remove failure");
    let notices = vec![
        LifecycleNotice {
            at: 10,
            found: 0,
            scanned: 15,
            error: None,
        },
        LifecycleNotice {
            at: 20,
            found: 1,
            scanned: 8,
            error: None,
        },
    ];
    std::fs::write(
        database::beside("antivirus.notices.json").expect("notices"),
        serde_json::to_vec(&notices).expect("notices"),
    )
    .expect("notice file");
    antivirus::resume(db.clone()).await;
    idle(&db).await;
    let before: (i64,) = sqlx::query_as("SELECT count(*) FROM notifications")
        .fetch_one(&db)
        .await
        .expect("notifications");
    antivirus::resume(db.clone()).await;
    idle(&db).await;
    let after: (i64,) = sqlx::query_as("SELECT count(*) FROM notifications")
        .fetch_one(&db)
        .await
        .expect("notifications");
    assert_eq!(before.0, after.0);
    config.enabled = true;
    settings::write(&db, Section::Antivirus, &config)
        .await
        .expect("enable saved");
    antivirus::resume(db.clone()).await;
    idle(&db).await;
    let keys: Vec<(String,)> =
        sqlx::query_as("SELECT key FROM notifications WHERE source = 'nas-security-center'")
            .fetch_all(&db)
            .await
            .expect("notifications");
    for expected in ["antivirusClean", "antivirusThreats", "antivirusFailed"] {
        assert!(keys.iter().any(|key| key.0 == expected));
    }
}
