use crate::support::{package, Checksum, Machine};
use nas_os_axum::{
    services::{database, packages, registry},
    widgets::services::tasks,
};
use sqlx::SqlitePool;
use std::time::Duration;

async fn database() -> SqlitePool {
    Machine::get();
    database::open_in_memory().await.expect("database")
}

async fn installed(db: &SqlitePool, id: &str) -> Option<String> {
    registry::installed(db)
        .await
        .expect("registry")
        .into_iter()
        .find(|app| app.id == id)
        .map(|app| app.version)
}

#[tokio::test]
async fn installs_an_application_from_github_and_from_gitlab() {
    let db = database().await;
    let machine = Machine::get();
    machine.publish(
        "tests/probe",
        package("probe", "1.2.0", &[("bin/app", "#!/bin/sh\n")]),
    );
    machine.publish("tests/labprobe", package("labprobe", "2.0.0", &[]));

    packages::install_for_setup(&db, "probe")
        .await
        .expect("probe installed");
    packages::install_for_setup(&db, "labprobe")
        .await
        .expect("labprobe installed");

    assert_eq!(installed(&db, "probe").await.as_deref(), Some("1.2.0"));
    assert_eq!(installed(&db, "labprobe").await.as_deref(), Some("2.0.0"));
    assert!(machine.root.join("apps/probe/bin/app").is_file());
    let log = std::fs::read_to_string(machine.root.join("logs/install-probe.log")).expect("log");
    assert!(log.contains("Installed version 1.2.0"));

    machine.publish("tests/probe", package("probe", "1.3.0", &[]));
    packages::install_for_setup(&db, "probe")
        .await
        .expect("probe updated");
    assert_eq!(installed(&db, "probe").await.as_deref(), Some("1.3.0"));
    assert!(!machine.root.join("apps/probe/bin/app").exists());
}

#[tokio::test]
async fn refuses_a_package_that_does_not_match_its_checksum_or_asks_too_much() {
    let db = database().await;
    let machine = Machine::get();
    let mut wrong = package("badsum", "1.0.0", &[]);
    wrong.checksum = Checksum::Wrong;
    machine.publish("tests/badsum", wrong);
    machine.publish(
        "tests/greedy",
        crate::support::Release {
            files: vec![(
                "nas-app.json",
                r#"{"manifest_version":1,"app":{"id":"greedy","name":"greedy"},"permissions":{"paths":["/etc"]}}"#.to_owned(),
            )],
            ..package("greedy", "1.0.0", &[])
        },
    );

    let checksum = packages::install_for_setup(&db, "badsum")
        .await
        .unwrap_err();
    assert!(checksum.to_lowercase().contains("checksum"), "{checksum}");
    let greedy = packages::install_for_setup(&db, "greedy")
        .await
        .unwrap_err();
    assert!(greedy.contains("/etc"), "{greedy}");
    assert!(!machine.root.join("apps/greedy").exists());
    assert!(!machine.root.join("apps/.greedy.new").exists());
    assert_eq!(installed(&db, "badsum").await, None);
}

#[tokio::test]
async fn says_what_the_release_lacks() {
    let db = database().await;
    let machine = Machine::get();
    machine.publish(
        "tests/noasset",
        crate::support::Release {
            asset: "something-else.zip".to_owned(),
            ..package("noasset", "1.0.0", &[])
        },
    );

    let asset = packages::install_for_setup(&db, "noasset")
        .await
        .unwrap_err();
    assert!(asset.contains("something-else.zip"), "{asset}");
    let release = packages::install_for_setup(&db, "norelease")
        .await
        .unwrap_err();
    assert!(release.contains("tests/norelease"), "{release}");
    let unknown = packages::install_for_setup(&db, "nowhere")
        .await
        .unwrap_err();
    assert!(unknown.contains("unknown application"), "{unknown}");
}

#[tokio::test]
async fn installs_in_the_background_and_warns_of_a_package_it_could_not_verify() {
    let db = database().await;
    let machine = Machine::get();
    let mut unverified = package("nosum", "1.0.0", &[]);
    unverified.checksum = Checksum::Missing;
    machine.publish("tests/nosum", unverified);

    let task = packages::start_install(&db, "nosum")
        .await
        .expect("started");
    for _ in 0..200 {
        if installed(&db, "nosum").await.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(installed(&db, "nosum").await.as_deref(), Some("1.0.0"));
    let warned: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE key = 'packageNotVerified'")
            .fetch_one(&db)
            .await
            .expect("notifications");
    assert_eq!(warned, 1);
    assert!(tasks::get(&db, &task).await.is_ok() || tasks::list(&db).await.is_ok());
}

#[tokio::test]
async fn checks_the_catalogue_against_the_forges_and_announces_what_is_newer() {
    let db = database().await;
    let machine = Machine::get();
    machine.publish("tests/checked", package("checked", "1.0.0", &[]));
    packages::install_for_setup(&db, "checked")
        .await
        .expect("installed");
    machine.publish("tests/checked", package("checked", "1.1.0", &[]));

    let offers = packages::check(&db).await.expect("checked");
    let checked = offers
        .iter()
        .find(|offer| offer.id == "checked")
        .expect("offer");
    assert_eq!(checked.installed_version.as_deref(), Some("1.0.0"));
    assert_eq!(checked.latest_version.as_deref(), Some("1.1.0"));
    assert!(checked.update_available);
    let norelease = offers
        .iter()
        .find(|offer| offer.id == "norelease")
        .expect("offer");
    assert_eq!(norelease.latest_version, None);

    packages::announce(&db, &offers).await;
    let announced: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE key = 'appUpdatesAvailable'")
            .fetch_one(&db)
            .await
            .expect("notifications");
    assert_eq!(announced, 1);
    assert!(!packages::offers(&db).await.expect("offers").is_empty());
}
