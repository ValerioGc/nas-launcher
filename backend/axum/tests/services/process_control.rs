use super::*;

fn process(id: u32, kernel: bool) -> Seen {
    Seen {
        id,
        name: "worker".to_owned(),
        started: 900,
        kernel,
    }
}

#[test]
fn the_unit_is_the_first_service_or_scope_of_the_cgroup() {
    assert_eq!(
        unit_in("0::/system.slice/nas-app@nas-file-manager.service/worker\n").as_deref(),
        Some("nas-app@nas-file-manager.service")
    );
    assert_eq!(
        unit_in("0::/user.slice/user-1000.slice/session-3.scope\n").as_deref(),
        Some("session-3.scope")
    );
    assert_eq!(unit_in("0::/\n"), None);
    assert_eq!(unit_in(""), None);
}

#[test]
fn what_keeps_the_nas_working_is_only_restarted() {
    let worker = process(400, false);
    let essential = allowed(&worker, Some("systemd-journald.service"));
    assert!(!essential.terminate);
    assert!(essential.restart);

    let ordinary = allowed(&worker, Some("cron.service"));
    assert!(ordinary.terminate && ordinary.restart);

    let session = allowed(&worker, Some("session-3.scope"));
    assert!(session.terminate);
    assert!(!session.restart);

    let helper = allowed(&worker, Some("nas-os-process@restart-1-2.service"));
    assert!(!helper.restart);
}

#[test]
fn the_kernel_and_the_first_process_are_left_alone() {
    let none = Allowed {
        terminate: false,
        restart: false,
    };
    assert_eq!(allowed(&process(50, true), None), none);
    assert_eq!(allowed(&process(1, false), Some("init.scope")), none);
}

#[test]
fn a_pattern_stands_for_any_text_in_the_middle_only() {
    assert!(is_essential("systemd-logind.service"));
    assert!(is_essential("nas-os.service"));
    assert!(!is_essential("systemd-.scope"));
    assert!(!is_essential("nas-os-gateway.socket"));
    assert!(!is_essential("cron.service"));
}

#[test]
fn the_helper_unit_names_the_action_the_process_and_its_start() {
    assert_eq!(
        helper_unit(Action::Terminate, 400, 900),
        "nas-os-process@terminate-400-900.service"
    );
    assert_eq!(
        helper_unit(Action::Restart, 1, 0),
        "nas-os-process@restart-1-0.service"
    );
}

#[test]
fn only_terminating_a_valid_nas_app_identifies_a_window_to_close() {
    let unit = Some("nas-app@nas-picture-manager.service");
    assert_eq!(
        terminated_app(Action::Terminate, unit).as_deref(),
        Some("nas-picture-manager")
    );
    assert_eq!(terminated_app(Action::Restart, unit), None);
    for unit in [
        None,
        Some("cron.service"),
        Some("session-3.scope"),
        Some("nas-app@.service"),
        Some("nas-app@../other.service"),
        Some("nas-app@nas-picture-manager.scope"),
    ] {
        assert_eq!(terminated_app(Action::Terminate, unit), None);
    }
    let result = ProcessResult {
        app_id: terminated_app(
            Action::Terminate,
            Some("nas-app@nas-picture-manager.service"),
        ),
    };
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::json!({ "appId": "nas-picture-manager" })
    );
}

#[test]
fn the_helper_knows_the_same_essential_units() {
    let script = include_str!("../../src/services/process_control/act-on-process.sh");
    let list = script
        .split_once("readonly ESSENTIAL=(\n")
        .and_then(|(_, rest)| rest.split_once("\n)"))
        .map(|(list, _)| list)
        .expect("the list in the script");
    let listed: Vec<&str> = list
        .lines()
        .map(|line| line.trim().trim_matches('\''))
        .collect();
    assert_eq!(listed, ESSENTIAL);
}

#[tokio::test]
async fn a_process_that_is_gone_or_changed_is_not_acted_on() {
    let request = |started| ProcessRequest {
        action: Action::Terminate,
        started,
    };
    let gone = act(u32::MAX, request(0))
        .await
        .expect_err("no such process");
    assert_eq!(gone.0, axum::http::StatusCode::NOT_FOUND);

    let changed = act(std::process::id(), request(u64::MAX))
        .await
        .expect_err("another start time");
    assert_eq!(changed.0, axum::http::StatusCode::NOT_FOUND);
}
