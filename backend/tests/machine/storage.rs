//! The disks, as the launcher hands them to its root units: S.M.A.R.T., a new name for a volume and
//! the initialization of a blank disk. The root helpers that mount and format are not run here.

use crate::support::Machine;
use nas_os_axum::{
    services::{
        database,
        storage::{self, InitializeRequest, RenameRequest, SmartRequest},
    },
    widgets::services::tasks,
};
use std::time::Duration;

#[tokio::test]
async fn reads_smart_only_of_a_data_disk() {
    let machine = Machine::get();
    let request = machine.root.join("smart-request.json");

    std::fs::write(&request, r#"{"device":"sdz"}"#).expect("request");
    storage::smart_reported(&request).expect("read");
    std::fs::write(&request, r#"{"device":"sda"}"#).expect("request");
    assert!(storage::smart_reported(&request).is_err());
    std::fs::write(&request, r#"{"device":"../sda"}"#).expect("request");
    assert!(storage::smart_reported(&request).is_err());

    // The root unit is a fake here: no result comes back, and the launcher says so.
    let asked = storage::smart_data(SmartRequest {
        device: "sdz".to_owned(),
    })
    .await;
    assert!(asked.is_err());
    assert!(storage::smart_data(SmartRequest {
        device: "/dev/sdz".to_owned()
    })
    .await
    .is_err());
    assert!(!machine
        .calls("systemctl start nas-os-storage-smart.service")
        .is_empty());
}

#[tokio::test]
async fn renames_only_an_ext_volume_of_data() {
    let machine = Machine::get();
    let db = database::open_in_memory().await.expect("database");
    let rename = |device: &str, label: &str| RenameRequest {
        device: device.to_owned(),
        label: label.to_owned(),
    };

    storage::rename_volume(&db, rename("sdy1", "Photos"))
        .await
        .expect("renamed");
    assert!(!machine
        .calls("systemctl start nas-os-storage-rename.service")
        .is_empty());
    machine.fail_unit("nas-os-storage-rename.service", Some("e2label refused"));
    assert!(storage::rename_volume(&db, rename("sdy1", "Photos"))
        .await
        .is_err());
    machine.fail_unit("nas-os-storage-rename.service", None);
    let failed: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE key = 'diskRenameFailed'")
            .fetch_one(&db)
            .await
            .expect("notifications");
    assert_eq!(failed, 1);
    for (device, label) in [("sda1", "Root"), ("sdx9", "Gone"), ("sdy1", "a/b")] {
        assert!(
            storage::rename_volume(&db, rename(device, label))
                .await
                .is_err(),
            "{device}"
        );
    }
}

#[tokio::test]
async fn initializes_only_a_blank_disk_and_follows_the_root_unit() {
    let machine = Machine::get();
    let db = database::open_in_memory().await.expect("database");
    let initialize = |device: &str| InitializeRequest {
        device: device.to_owned(),
        label: "Data".to_owned(),
    };

    let refused = storage::start_initialize(&db, initialize("sdy")).await;
    assert!(refused.is_err());
    let failed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE key = 'diskInitializationFailed'",
    )
    .fetch_one(&db)
    .await
    .expect("notifications");
    assert_eq!(failed, 1);
    assert!(storage::start_initialize(&db, initialize(".bad"))
        .await
        .is_err());

    let accepted = storage::start_initialize(&db, initialize("sdz"))
        .await
        .expect("accepted");
    for _ in 0..200 {
        let task = tasks::get(&db, &accepted.task_id).await.expect("task");
        if !matches!(task.state, tasks::TaskState::Running) {
            assert!(matches!(task.state, tasks::TaskState::Done));
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(!machine
        .calls("systemctl start nas-os-storage-initialize.service")
        .is_empty());

    // The root unit fails: the task says why.
    machine.fail_unit("nas-os-storage-initialize.service", Some("mkfs failed"));
    let again = storage::start_initialize(&db, initialize("sdz"))
        .await
        .expect("accepted");
    let mut state = None;
    for _ in 0..200 {
        let task = tasks::get(&db, &again.task_id).await.expect("task");
        if !matches!(task.state, tasks::TaskState::Running) {
            state = Some(task.state);
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    machine.fail_unit("nas-os-storage-initialize.service", None);
    assert!(matches!(state, Some(tasks::TaskState::Failed)));
}

#[tokio::test]
async fn ejects_only_an_external_disk_that_is_not_marked() {
    use nas_os_axum::services::storage::eject::{self, EjectRequest, LockRequest};
    let machine = Machine::get();
    let db = database::open_in_memory().await.expect("database");
    let eject = |device: &str| EjectRequest {
        device: device.to_owned(),
    };

    eject::eject(&db, eject("sdu1")).await.expect("ejected");
    machine.fail_unit("nas-os-storage-eject.service", Some("busy"));
    assert!(eject::eject(&db, eject("sdu1")).await.is_err());
    machine.fail_unit("nas-os-storage-eject.service", None);
    assert!(!machine
        .calls("systemctl start nas-os-storage-eject.service")
        .is_empty());
    for device in ["sda", "sdz", "sdq", "../sdu"] {
        assert!(eject::eject(&db, eject(device)).await.is_err(), "{device}");
    }

    let marked = eject::set_lock(
        &db,
        LockRequest {
            uuid: "ABCD-1234".to_owned(),
            locked: true,
        },
    )
    .await
    .expect("marked");
    assert_eq!(marked, ["abcd-1234"]);
    assert!(eject::eject(&db, eject("sdu")).await.is_err());
    assert!(eject::set_lock(
        &db,
        LockRequest {
            uuid: "not valid!".to_owned(),
            locked: true
        }
    )
    .await
    .is_err());
    let unmarked = eject::set_lock(
        &db,
        LockRequest {
            uuid: "abcd-1234".to_owned(),
            locked: false,
        },
    )
    .await
    .expect("unmarked");
    assert!(unmarked.is_empty());
}
