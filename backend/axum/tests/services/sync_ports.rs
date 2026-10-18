use super::*;
use crate::services::manifest::Capability;

fn manifest(network: &[&str]) -> Manifest {
    Manifest {
        id: "nas-sync-manager".to_owned(),
        name: "Sync".to_owned(),
        system: false,
        version: "1.0.0".to_owned(),
        icon: String::new(),
        category: None,
        protocol: 1,
        provides: Vec::<Capability>::new(),
        paths: Vec::new(),
        network: network.iter().map(|kind| (*kind).to_owned()).collect(),
        packages: Vec::new(),
        background: false,
        shares: false,
        cleanup: false,
        socket: PathBuf::new(),
        exec: String::new(),
    }
}

#[test]
fn only_an_application_that_asks_for_the_sync_opens_its_ports() {
    assert!(may_sync(&manifest(&["sync"])));
    assert!(may_sync(&manifest(&["smtp", "sync"])));
    assert!(!may_sync(&manifest(&["smtp"])));
    assert!(!may_sync(&manifest(&[])));
}

#[tokio::test]
async fn an_application_that_does_not_declare_it_opens_nothing() {
    let refused = declare("nas-not-installed", true)
        .await
        .expect_err("refused");
    assert_eq!(refused.0, axum::http::StatusCode::FORBIDDEN);

    refresh().await;
    revoke("nas-not-installed").await;
    sync(&[manifest(&["sync"])]).await;
}

#[tokio::test]
async fn a_failed_application_is_retried_even_when_its_request_has_not_changed() {
    let directory =
        std::env::temp_dir().join(format!("nas-sync-ports-retry-{}", std::process::id()));
    let _ = tokio::fs::remove_dir_all(&directory).await;
    let request = directory.join("nas-sync-manager");
    let failure = apply_request(&directory, "nas-sync-manager", true, async {
        Err(ApiFailure::unavailable("firewall not ready"))
    })
    .await;
    assert!(failure.is_err());
    assert_eq!(tokio::fs::read_to_string(&request).await.unwrap(), "on");
    let applied = std::sync::atomic::AtomicBool::new(false);
    apply_request(&directory, "nas-sync-manager", true, async {
        applied.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    })
    .await
    .unwrap();
    assert!(applied.load(std::sync::atomic::Ordering::Relaxed));

    assert!(apply_request(&directory, "nas-sync-manager", false, async {
        Err(ApiFailure::unavailable("firewall not ready"))
    })
    .await
    .is_err());
    applied.store(false, std::sync::atomic::Ordering::Relaxed);
    apply_request(&directory, "nas-sync-manager", false, async {
        assert!(!request.exists());
        applied.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    })
    .await
    .unwrap();
    assert!(applied.load(std::sync::atomic::Ordering::Relaxed));
    tokio::fs::remove_dir_all(directory).await.unwrap();
}
