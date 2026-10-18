use super::*;

#[test]
fn a_unit_is_named_after_the_application_and_only_a_real_identifier_gets_one() {
    assert_eq!(
        unit_of("fileExplorer").as_deref(),
        Some("nas-app@fileExplorer.service")
    );

    for id in [
        "",
        "../etc",
        "file explorer",
        "a;systemctl",
        &"x".repeat(65),
    ] {
        assert!(unit_of(id).is_none(), "{id} is not an identifier");
    }
}

#[test]
fn only_three_verbs_exist_and_they_are_the_ones_polkit_grants() {
    assert_eq!(Action::parse("start").map(Action::verb), Some("start"));
    assert_eq!(Action::parse("stop").map(Action::verb), Some("stop"));
    assert_eq!(Action::parse("restart").map(Action::verb), Some("restart"));
    assert!(Action::parse("enable").is_none());
    assert!(Action::parse("mask").is_none());
}

#[test]
fn only_inactive_and_failed_units_are_considered_stopped() {
    assert!(stopped_state("inactive"));
    assert!(stopped_state("failed"));
    assert!(!stopped_state("active"));
    assert!(!stopped_state("deactivating"));
}

#[tokio::test]
async fn nothing_is_started_where_the_mode_is_not_real() {
    let failure = run("fileExplorer", Action::Start)
        .await
        .expect_err("refused without the mode");

    assert!(failure.1.contains("not available on this installation"));
}

#[tokio::test]
async fn every_application_is_reported_as_stopped_where_the_mode_is_not_real() {
    let state = running(&["fileExplorer".to_owned(), "mediaServer".to_owned()]).await;

    assert_eq!(state.get("fileExplorer"), Some(&false));
    assert_eq!(state.get("mediaServer"), Some(&false));
}

#[tokio::test]
async fn a_started_application_is_ready_only_after_its_socket_accepts_connections() {
    let path = std::env::temp_dir().join(format!("nas-app-ready-{}.sock", std::process::id()));
    let _ = tokio::fs::remove_file(&path).await;
    let delayed_path = path.clone();
    let listener = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let listener = tokio::net::UnixListener::bind(delayed_path).expect("socket is available");
        tokio::time::sleep(Duration::from_millis(200)).await;
        listener
    });

    assert!(wait_for_socket(&path, Duration::from_millis(180)).await);
    listener.await.expect("listener task finished");
    tokio::fs::remove_file(&path)
        .await
        .expect("socket is removed");
    assert!(!wait_for_socket(&path, Duration::from_millis(50)).await);
}
