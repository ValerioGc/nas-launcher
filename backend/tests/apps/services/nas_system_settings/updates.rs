use super::*;

#[test]
fn a_finished_update_says_which_version_is_running() {
    let (notice, tone) = outcome("done", "0.2.0");

    assert_eq!(notice.key, "launcherUpdated");
    assert_eq!(notice.title, "Launcher updated");
    assert_eq!(
        notice.message,
        "The launcher was updated to the latest version (0.2.0)."
    );
    assert_eq!(notice.params["version"], "0.2.0");
    assert_eq!(tone, Tone::Success);
}

#[test]
fn an_update_of_more_components_names_them() {
    assert_eq!(outcome("done launcher", "0.2.0").0.key, "launcherUpdated");
    let (notice, tone) = outcome("done wizard gateway launcher", "0.2.0");
    assert_eq!(notice.key, "nasUpdated");
    assert_eq!(notice.params["components"], "wizard, gateway, launcher");
    assert_eq!(
        notice.message,
        "The NAS was updated (wizard, gateway, launcher). Running launcher version (0.2.0)."
    );
    assert_eq!(tone, Tone::Success);
}

#[test]
fn the_gateway_and_the_wizard_come_from_the_repositories_beside_the_launcher() {
    assert_eq!(
        sibling("ValerioGc/nas-launcher", "nas-gateway"),
        "ValerioGc/nas-gateway"
    );
    assert_eq!(sibling("", "nas-wizard"), "");
    assert_eq!(Component::Gateway.archive(), "gateway-update.tar");
    assert!(Component::Wizard < Component::Gateway && Component::Gateway < Component::Launcher);
}

#[test]
fn a_component_whose_version_is_unknown_takes_any_release() {
    assert!(is_update("0.1.15", None));
    assert!(is_update("0.1.15", Some("0.1.14")));
    assert!(!is_update("0.1.14", Some("0.1.14")));
    assert!(!is_update("0.1.13", Some("0.1.14")));
}

#[test]
fn a_newer_gateway_or_wizard_has_a_notice_of_its_own() {
    let notice = available_notice(Component::Gateway, "0.1.15", None);
    assert_eq!(notice.key, "componentUpdateAvailable");
    assert_eq!(notice.params["component"], "gateway");
    assert_eq!(notice.params["current"], "-");
    let notice = available_notice(Component::Launcher, "0.2.0", Some("0.1.0"));
    assert_eq!(notice.key, "launcherUpdateAvailable");
}

#[tokio::test]
async fn the_version_installed_is_read_from_the_binary_of_each_component() {
    assert_eq!(
        Component::Launcher.installed().await,
        Some(Some(env!("CARGO_PKG_VERSION").to_owned()))
    );
    std::env::set_var("NAS_WIZARD_BINARY", "/no/such/wizard");
    assert_eq!(Component::Wizard.installed().await, None);
    std::env::set_var("NAS_GATEWAY_BINARY", "/bin/false");
    assert_eq!(Component::Gateway.installed().await, Some(None));
}

#[test]
fn a_rolled_back_or_refused_update_says_why() {
    let (notice, tone) = outcome("rolled-back the new launcher does not answer", "0.1.6");
    assert_eq!(notice.key, "launcherRolledBack");
    assert!(notice
        .message
        .ends_with("(the new launcher does not answer)."));
    assert_eq!(notice.params["reason"], "the new launcher does not answer");
    assert_eq!(tone, Tone::Error);

    let (notice, _) = outcome("failed the release has no installer", "0.1.6");
    assert_eq!(notice.key, "launcherUpdateFailed");
    assert_eq!(notice.params["reason"], "the release has no installer");
}

#[test]
fn an_interrupted_update_says_where_it_stopped() {
    let (notice, tone) = outcome("installing", "0.1.6");

    assert_eq!(notice.key, "launcherUpdateInterrupted");
    assert_eq!(notice.params["step"], "installing");
    assert!(notice.message.contains("during the installing step"));
    assert_eq!(tone, Tone::Warning);
}

async fn recorded(db: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar("SELECT key FROM notifications ORDER BY id")
        .fetch_all(db)
        .await
        .expect("notifications")
}

fn status_file(name: &str, status: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("nas-os-{name}-{}.status", std::process::id()));
    std::fs::write(&path, status).expect("status");
    path
}

#[tokio::test]
async fn a_launcher_started_by_its_own_update_waits_for_the_end_instead_of_a_warning() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("db");
    let path = status_file("settle", "installing\n");
    let checks = std::sync::atomic::AtomicU32::new(0);
    let writer = path.clone();

    settle(&db, &path, Duration::from_millis(1), 50, || {
        // The update unit writes `done` while the new launcher waits.
        if checks.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 2 {
            std::fs::write(&writer, "done\n").expect("status");
        }
        async { true }
    })
    .await;

    assert_eq!(recorded(&db).await, ["launcherUpdated"]);
    assert!(!path.exists());
}

#[tokio::test]
async fn a_step_left_with_no_update_running_is_a_real_interruption() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("db");
    let path = status_file("interrupted", "installing\n");

    settle(&db, &path, Duration::from_millis(1), 50, || async { false }).await;

    assert_eq!(recorded(&db).await, ["launcherUpdateInterrupted"]);
    settle(&db, &path, Duration::from_millis(1), 50, || async { false }).await;
    assert_eq!(recorded(&db).await.len(), 1);
}
