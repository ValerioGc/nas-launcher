//! The applications on the machine: started, stopped and given what they ask for through the root
//! units, and the other requests the settings hand to systemd.

use crate::support::{package, Machine, Release};
use nas_os_axum::{
    apps::services::nas_app_manager,
    apps::services::nas_system_settings::{
        access::{self, NewRule, RuleKind},
        network::{self, WifiRequest},
        reset,
    },
    services::{
        background, database, grants, lifecycle,
        lifecycle::Action,
        manifest,
        packages::{self, credentials},
        process_control::{self, Action as ProcessAction, ProcessRequest},
        system_packages,
    },
};

#[tokio::test]
async fn starts_stops_and_cleans_an_application_through_its_unit() {
    let machine = Machine::get();
    let db = database::open_in_memory().await.expect("database");
    machine.publish("tests/started", package("started", "1.0.0", &[]));
    packages::install_for_setup(&db, "started")
        .await
        .expect("installed");
    let socket = manifest::socket_of("started");
    let _ = std::fs::remove_file(&socket);

    // Stopped: systemd starts it, and it is ready once its socket answers.
    let bound = socket.clone();
    let late = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        tokio::net::UnixListener::bind(bound).expect("socket")
    });
    lifecycle::run("started", Action::Start)
        .await
        .expect("started");
    let listener = late.await.expect("listener");
    assert_eq!(
        machine
            .calls("systemctl start nas-app@started.service")
            .len(),
        1
    );
    // Answering already: systemd is not asked again.
    lifecycle::run("started", Action::Start)
        .await
        .expect("running");
    assert_eq!(
        machine
            .calls("systemctl start nas-app@started.service")
            .len(),
        1
    );
    lifecycle::run("started", Action::Stop)
        .await
        .expect("stopped");
    machine.activate("nas-app@started.service", true);
    assert!(lifecycle::is_running("started").await);
    let states = lifecycle::running(&["started".to_owned(), "../bad".to_owned()]).await;
    assert!(states["started"]);
    assert!(!states["../bad"]);
    lifecycle::restart_after_install(&db, "started", true).await;
    machine.activate("nas-app@started.service", false);
    lifecycle::stop_before_removal("started").await;
    lifecycle::remove_data("started").await;
    drop(listener);

    // An application with no manifest does not start: an interactive install says so.
    lifecycle::restart_after_install(&db, "ghost", true).await;
    lifecycle::restart_after_install(&db, "ghost", false).await;
    let unstarted: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE key = 'appNotStarted'")
            .fetch_one(&db)
            .await
            .expect("notifications");
    assert_eq!(unstarted, 1);
    assert!(lifecycle::run("../bad", Action::Start).await.is_err());
    assert!(lifecycle::run("ghost", Action::Start).await.is_err());
    assert!(!machine
        .calls("systemctl start nas-app@started.service")
        .is_empty());
    assert!(!machine
        .calls("systemctl clean --what=state nas-app@started.service")
        .is_empty());
}

#[tokio::test]
async fn grants_paths_and_system_packages_through_the_root_helpers() {
    let machine = Machine::get();

    assert!(grants::apply("granted", &["/srv/storage"])
        .await
        .expect("applied"));
    grants::revoke("granted").await.expect("revoked");
    assert!(!machine
        .calls("systemctl start nas-os-app-grants@granted.service")
        .is_empty());

    system_packages::install("mailer", &["msmtp"])
        .await
        .expect("installed");
    system_packages::remove("mailer").await.expect("removed");
    system_packages::remove("mailer")
        .await
        .expect("nothing left to remove");
    assert_eq!(
        machine
            .calls("systemctl start nas-os-app-packages@mailer.service")
            .len(),
        2
    );
}

#[tokio::test]
async fn stores_the_token_and_has_the_firewall_follow_the_allowed_addresses() {
    let machine = Machine::get();
    let db = database::open_in_memory().await.expect("database");

    credentials::store("ghp_example_token")
        .await
        .expect("stored");
    assert!(credentials::store("not a token").await.is_err());
    assert!(!machine
        .calls("systemctl start --no-block nas-os-token.service")
        .is_empty());

    let rules = access::add(
        &db,
        NewRule {
            kind: RuleKind::Range,
            value: "192.168.1.0/24".to_owned(),
            label: "home".to_owned(),
        },
    )
    .await
    .expect("added");
    assert_eq!(rules.len(), 1);
    assert!(access::allows(&db, "192.168.1.20".parse().expect("address")).await);
    assert!(!access::allows(&db, "10.1.1.1".parse().expect("address")).await);
    assert!(access::allows(&db, "127.0.0.1".parse().expect("address")).await);
    access::apply_saved(&db).await;
    access::record_failure(&db, Some("10.1.1.1".parse().expect("address")), "password").await;
    access::record_failure(&db, None, "username").await;
    assert!(access::remove(&db, &rules[0].id)
        .await
        .expect("removed")
        .is_empty());
    assert!(!machine
        .calls("systemctl start nas-os-firewall.service")
        .is_empty());
}

#[tokio::test]
async fn terminates_a_process_through_its_helper_unit() {
    let machine = Machine::get();
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("sleep");
    let id = child.id();
    let stat = std::fs::read_to_string(format!("/proc/{id}/stat")).expect("stat");
    let started: u64 = stat
        .rsplit_once(") ")
        .expect("fields")
        .1
        .split_whitespace()
        .nth(19)
        .expect("start time")
        .parse()
        .expect("number");

    let stale = ProcessRequest {
        action: ProcessAction::Terminate,
        started: started + 1,
    };
    assert!(process_control::act(id, stale).await.is_err());
    let result = process_control::act(
        id,
        ProcessRequest {
            action: ProcessAction::Terminate,
            started,
        },
    )
    .await;
    child.kill().expect("kill");
    let _ = child.wait();
    if result.is_ok() {
        assert!(!machine
            .calls(&format!("systemctl start nas-os-process@terminate-{id}-"))
            .is_empty());
    }
}

#[tokio::test]
async fn starts_the_reset_and_changes_the_network_through_nmcli() {
    let _operations = crate::support::ROOT_OPERATIONS.lock().await;
    let machine = Machine::get();
    let db = database::open_in_memory().await.expect("database");

    let state = reset::state().await;
    assert_eq!(state.reason, None);
    reset::start().await.expect("reset started");
    assert!(!machine
        .calls("systemctl start --no-block nas-os-reset.service")
        .is_empty());

    // The fake nmcli joins what it is asked to, when the machine reads its network at all.
    let wifi = network::connect_wifi(
        &db,
        WifiRequest {
            ssid: "home".to_owned(),
            password: Some("long enough".to_owned()),
        },
    )
    .await;
    let wired = network::connect_wired(&db).await;
    // Left only when the Wi-Fi is not the way the launcher is reached: either answer is right.
    let _ = network::disconnect_wifi(&db).await;
    if network::status(&db, false).await.is_ok() {
        assert!(wifi.is_ok() && wired.is_ok());
        assert!(!machine
            .calls("nmcli device wifi connect home ifname wlan9")
            .is_empty());
    }
}

#[tokio::test]
async fn restarts_only_a_process_of_a_service() {
    Machine::get();
    let gone = process_control::act(
        u32::MAX - 1,
        ProcessRequest {
            action: ProcessAction::Restart,
            started: 0,
        },
    )
    .await;
    assert!(gone.is_err());

    let own = std::process::id();
    let stat = std::fs::read_to_string(format!("/proc/{own}/stat")).expect("stat");
    let started: u64 = stat
        .rsplit_once(") ")
        .expect("fields")
        .1
        .split_whitespace()
        .nth(19)
        .expect("start time")
        .parse()
        .expect("number");
    // This test runs in a session or a scope, which systemd does not restart.
    let restarted = process_control::act(
        own,
        ProcessRequest {
            action: ProcessAction::Restart,
            started,
        },
    )
    .await;
    assert!(restarted.is_err() || std::env::var_os("INVOCATION_ID").is_some());
}

#[tokio::test]
async fn keeps_running_without_its_window_only_what_has_work_to_do() {
    let machine = Machine::get();
    let db = database::open_in_memory().await.expect("database");
    machine.publish(
        "tests/worker",
        Release {
            files: vec![(
                "nas-app.json",
                r#"{"manifest_version":1,"app":{"id":"worker","name":"worker"},"run":{"background":true}}"#
                    .to_owned(),
            )],
            ..package("worker", "1.0.0", &[])
        },
    );
    packages::install_for_setup(&db, "worker")
        .await
        .expect("installed");
    let state = std::path::PathBuf::from(std::env::var("NAS_DB_PATH").expect("database path"));
    let request = state.with_file_name("app-grants").join("worker");
    assert_eq!(
        std::fs::read_to_string(request).expect("grants request"),
        "run:background\n"
    );

    // Nothing to do yet: its window closing stops it.
    let closes = || machine.calls("systemctl stop nas-app@worker.service").len();
    let before = closes();
    nas_app_manager::control(&db, "worker".to_owned(), "close")
        .await
        .expect("closed");
    assert_eq!(closes(), before + 1);

    // Work to do: started at boot, left running when its window closes.
    background::declare(&db, "worker", true)
        .await
        .expect("declared");
    assert!(background::keeps_running(&db, "worker").await);
    let socket = manifest::socket_of("worker");
    let _ = std::fs::remove_file(&socket);
    let bound = socket.clone();
    let late = tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        tokio::net::UnixListener::bind(bound).expect("socket")
    });
    background::start(db.clone()).await;
    let listener = late.await.expect("listener");
    assert_eq!(
        machine
            .calls("systemctl start nas-app@worker.service")
            .len(),
        1
    );
    let ack = nas_app_manager::control(&db, "worker".to_owned(), "close")
        .await
        .expect("closed");
    assert_eq!(ack.action, "close");
    assert_eq!(closes(), before + 1);
    drop(listener);

    // An application that does not declare it is refused, and nothing is done with it.
    let refused = background::declare(&db, "started", true)
        .await
        .expect_err("refused");
    assert_eq!(refused.0, axum::http::StatusCode::FORBIDDEN);
    background::declare(&db, "worker", false)
        .await
        .expect("declared");
    assert!(!background::keeps_running(&db, "worker").await);
}
