//! The update of the NAS: the releases of the launcher, the gateway and the wizard, checked,
//! downloaded beside the database and handed to the unit that installs them.

use crate::support::{Checksum, Machine, Release};
use nas_os_axum::{
    apps::services::nas_system_settings::updates::{self, Component},
    services::{database, session::SessionData},
    widgets::services::tasks,
};
use std::time::Duration;

fn release(name: &str, version: &'static str, checksum: Checksum) -> Release {
    Release {
        version,
        asset: format!("{name}-{version}-linux-x64.tar.gz"),
        files: vec![("install.sh", "#!/bin/sh\n".to_owned())],
        checksum,
    }
}

#[tokio::test]
async fn checks_downloads_and_hands_the_update_of_the_nas_over() {
    let machine = Machine::get();
    let db = database::open_in_memory().await.expect("database");
    machine.publish(
        "tests/nas-launcher",
        release("nas-os", "9.0.0", Checksum::Right),
    );
    machine.publish(
        "tests/nas-gateway",
        release("nas-gateway", "0.2.0", Checksum::Right),
    );
    machine.publish(
        "tests/nas-wizard",
        release("nas-wizard", "0.1.0", Checksum::Right),
    );
    let mut session = SessionData::default();

    let found = updates::check(&db, &mut session).await.expect("checked");
    let available = |component: Component| {
        found
            .components
            .iter()
            .find(|update| update.component == component)
            .and_then(|update| update.available_version.clone())
    };
    assert_eq!(available(Component::Launcher).as_deref(), Some("9.0.0"));
    assert_eq!(available(Component::Gateway).as_deref(), Some("0.2.0"));
    assert_eq!(available(Component::Wizard), None);
    assert_eq!(found.components.len(), 3);
    assert_eq!(updates::state(&session).await.components.len(), 3);

    let task = updates::install_updates(&db, &mut session)
        .await
        .expect("started");
    let mut state = None;
    for _ in 0..200 {
        let current = tasks::get(&db, &task).await.expect("task").state;
        if !matches!(current, tasks::TaskState::Running) {
            state = Some(current);
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        matches!(state, Some(tasks::TaskState::Done)),
        "the update did not hand over"
    );
    assert!(database::beside("launcher-update.tar")
        .expect("archive")
        .is_file());
    assert!(database::beside("gateway-update.tar")
        .expect("archive")
        .is_file());
    assert!(!machine
        .calls("systemctl start --no-block nas-os-update.service")
        .is_empty());

    // A release without its checksum is not downloaded at all.
    machine.publish(
        "tests/nas-gateway",
        release("nas-gateway", "0.3.0", Checksum::Missing),
    );
    let refused = updates::download(Component::Gateway, "0.3.0")
        .await
        .unwrap_err();
    assert!(refused.contains("no checksum"), "{refused}");
    assert!(updates::install_updates(&db, &mut SessionData::default())
        .await
        .is_err());
}

#[tokio::test]
async fn reports_how_the_last_update_ended() {
    Machine::get();
    let db = database::open_in_memory().await.expect("database");
    let status = database::beside("launcher-update.status").expect("status");
    std::fs::write(&status, "done launcher\n").expect("status");

    updates::settle(&db, &status, Duration::from_millis(1), 2, || async {
        false
    })
    .await;
    updates::report_previous(&db).await;

    assert!(!status.exists());
    let reported: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE key = 'launcherUpdated'")
            .fetch_one(&db)
            .await
            .expect("notifications");
    assert_eq!(reported, 1);
}
