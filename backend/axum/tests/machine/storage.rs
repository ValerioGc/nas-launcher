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
async fn lists_the_disks_of_one_kind_without_the_system() {
    Machine::get();

    let external = storage::inventory::disks(Some("external")).expect("external disks");
    assert_eq!(
        external
            .iter()
            .map(|disk| disk.id.as_str())
            .collect::<Vec<_>>(),
        ["sdu1"]
    );
    let local = storage::inventory::disks(Some("local")).expect("local disks");
    assert!(local.iter().all(|disk| disk.kind == "local"));
    assert!(local
        .iter()
        .any(|disk| disk.id == "sdz" && disk.initializable));
    assert!(local.iter().all(|disk| !disk.id.starts_with("sda")));
}

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
        identity: "//0".to_owned(),
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
    let renamed = InitializeRequest {
        identity: "0x5001/OTHER/100".to_owned(),
        ..initialize("sdz")
    };
    assert!(storage::start_initialize(&db, renamed).await.is_err());

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

async fn serve_root_helper(socket: std::path::PathBuf) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let _ = std::fs::remove_file(&socket);
    let listener = tokio::net::UnixListener::bind(&socket).expect("socket");
    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        let mut request = Vec::new();
        stream.read_to_end(&mut request).await.expect("request");
        let mut helper = tokio::process::Command::new(env!("CARGO_BIN_EXE_nas-os-axum"))
            .arg("--storage-crypt")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("root helper");
        let mut input = helper.stdin.take().expect("input");
        input.write_all(&request).await.expect("written");
        drop(input);
        let answer = helper.wait_with_output().await.expect("answer");
        stream.write_all(&answer.stdout).await.expect("answered");
        let _ = stream.shutdown().await;
    }
}

#[tokio::test]
async fn keys_an_encrypted_volume_through_the_root_helper_without_showing_its_secrets() {
    use nas_os_axum::services::storage::crypt::{self, Request};
    let machine = root_helper().await;
    let db = database::open_in_memory().await.expect("database");
    std::fs::write(
        machine.root.join("answers/cryptsetup@luksDump"),
        r#"{"keyslots":{"0":{},"1":{}}}"#,
    )
    .expect("slots");
    let credential = machine.root.join("credstore/nas-os.luks-luks-w");
    std::fs::write(&credential, "sealed").expect("credential");

    let recovery = crypt::request(
        &db,
        Request::Recovery {
            uuid: "LUKS-W".to_owned(),
            current: None,
        },
    )
    .await
    .expect("recovery key")
    .recovery
    .expect("shown once");
    assert_eq!(recovery.len(), 71);
    assert!(!machine
        .calls("systemd-creds decrypt --name=nas-os.luks-luks-w")
        .is_empty());
    assert!(machine
        .calls("cryptsetup luksAddKey --batch-mode --key-file")
        .iter()
        .any(|call| call.contains("--new-key-slot 2 --pbkdf pbkdf2")));

    crypt::request(
        &db,
        Request::Password {
            uuid: "LUKS-W".to_owned(),
            current: Some("the old password".to_owned()),
            password: "the new password".to_owned(),
        },
    )
    .await
    .expect("password changed");
    assert!(!machine
        .calls("cryptsetup luksChangeKey --batch-mode --key-file")
        .is_empty());

    crypt::request(
        &db,
        Request::Automatic {
            uuid: "LUKS-W".to_owned(),
            enabled: false,
            password: None,
        },
    )
    .await
    .expect("automatic key removed");
    assert!(!credential.exists());
    assert!(!machine
        .calls("cryptsetup luksKillSlot --batch-mode /dev/sdw1 1")
        .is_empty());

    let calls = std::fs::read_to_string(machine.root.join("calls")).expect("calls");
    for secret in ["the old password", "the new password", recovery.as_str()] {
        assert!(!calls.contains(secret), "{secret} in an argument");
    }
    assert!(std::fs::read_dir(machine.root.join("runtime"))
        .expect("runtime")
        .next()
        .is_none());

    let listed = storage::inventory::disks(Some("local")).expect("disks");
    let container = listed
        .iter()
        .find(|disk| disk.id == "sdw1")
        .expect("container");
    assert_eq!(container.encryption, Some("open"));
    let keys = container.keys.expect("keys");
    assert!(keys.password && !keys.automatic && !keys.held);

    std::fs::create_dir_all(machine.root.join("fail/cryptsetup@/dev")).expect("fail folder");
    std::fs::write(
        machine.root.join("fail/cryptsetup@/dev/sdw1"),
        "No key available",
    )
    .expect("fail");
    let refused = crypt::request(
        &db,
        Request::Password {
            uuid: "LUKS-W".to_owned(),
            current: Some("not the password".to_owned()),
            password: "the new password".to_owned(),
        },
    )
    .await;
    std::fs::remove_file(machine.root.join("fail/cryptsetup@/dev/sdw1")).expect("cleared");
    assert!(refused.is_err());
    for uuid in ["LUKS-GONE", "INNER-W"] {
        assert!(crypt::request(
            &db,
            Request::Header {
                uuid: uuid.to_owned()
            }
        )
        .await
        .is_err());
    }
}

async fn root_helper() -> &'static Machine {
    static SERVED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    let machine = Machine::get();
    let socket = machine.root.join("crypt.sock");
    SERVED.get_or_init(|| {
        let served = socket.clone();
        std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .expect("runtime")
                .block_on(serve_root_helper(served));
        });
    });
    for _ in 0..100 {
        if socket.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    machine
}

#[tokio::test]
async fn formats_a_data_disk_encrypted_and_follows_the_task() {
    use nas_os_axum::services::storage::crypt::{self, Request};
    let machine = root_helper().await;
    let db = database::open_in_memory().await.expect("database");
    std::fs::write(machine.root.join("answers/cryptsetup@luksUUID"), "NEW-V").expect("uuid");
    std::fs::write(machine.root.join("answers/systemd-creds@encrypt"), "sealed").expect("sealed");
    let format = |device: &str| Request::Format {
        device: device.to_owned(),
        identity: "//0".to_owned(),
        label: "Vault".to_owned(),
        encrypt: true,
        automatic: true,
        password: Some("a long password".to_owned()),
    };

    for refused in ["sda", "sdu", "sdq"] {
        assert!(
            crypt::request(&db, format(refused)).await.is_err(),
            "{refused}"
        );
    }
    let task = crypt::request(&db, format("sdv"))
        .await
        .expect("accepted")
        .task_id
        .expect("task");
    let mut state = None;
    for _ in 0..500 {
        let current = tasks::get(&db, &task).await.expect("task");
        if !matches!(current.state, tasks::TaskState::Running) {
            state = Some((current.state, current.detail));
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let (state, detail) = state.expect("the format ended");
    assert!(matches!(state, tasks::TaskState::Done), "{detail}");

    for step in [
        "wipefs --all /dev/sdv1",
        "wipefs --all /dev/sdv",
        "parted --script --align optimal /dev/sdv mklabel gpt",
        "cryptsetup luksFormat --type luks2 --batch-mode --key-file",
        "cryptsetup luksAddKey --batch-mode --key-file",
        "cryptsetup open --key-file",
        "mkfs.ext4 -F -L Vault /dev/mapper/nas-luks-new-v",
    ] {
        assert!(!machine.calls(step).is_empty(), "{step}");
    }
    assert_eq!(
        std::fs::read(machine.root.join("credstore/nas-os.luks-new-v")).expect("credential"),
        b"sealed"
    );
    let calls = std::fs::read_to_string(machine.root.join("calls")).expect("calls");
    assert!(!calls.contains("a long password"));
}

#[tokio::test]
async fn unlocks_and_locks_a_volume_which_then_stays_closed() {
    use nas_os_axum::services::storage::crypt::{self, Request};
    let machine = root_helper().await;
    let db = database::open_in_memory().await.expect("database");
    let held =
        || std::fs::read_to_string(machine.root.join("state/storage-held")).unwrap_or_default();

    crypt::request(
        &db,
        Request::Lock {
            uuid: "LUKS-S".to_owned(),
        },
    )
    .await
    .expect("locked");
    assert!(held().contains("luks-s"));
    assert!(!machine.calls("cryptsetup close nas-luks-luks-s").is_empty());

    crypt::request(
        &db,
        Request::Unlock {
            uuid: "LUKS-T".to_owned(),
            password: "the password of t".to_owned(),
        },
    )
    .await
    .expect("unlocked");
    assert!(!machine
        .calls("cryptsetup open --key-file")
        .iter()
        .any(|call| call.contains("the password of t")));
    assert!(machine
        .calls("cryptsetup open --key-file")
        .iter()
        .any(|call| call.ends_with("/dev/sdt1 nas-luks-luks-t")));

    crypt::request(
        &db,
        Request::Unlock {
            uuid: "LUKS-S".to_owned(),
            password: "the password of s".to_owned(),
        },
    )
    .await
    .expect("opened by the person again");
    assert!(!held().contains("luks-s"));
}

#[tokio::test]
async fn keeps_tries_and_forgets_a_network_disk_through_the_root_helper() {
    use nas_os_axum::services::storage::network::{self, NetworkRequest};
    let machine = root_helper().await;
    std::fs::write(machine.root.join("answers/systemd-creds@encrypt"), "sealed").expect("sealed");
    let save = |password: Option<&str>| NetworkRequest::NetworkSave {
        id: None,
        name: "Office".to_owned(),
        server: "192.168.1.20".to_owned(),
        share: "Photos".to_owned(),
        username: "valerio".to_owned(),
        domain: None,
        password: password.map(str::to_owned),
        automatic: false,
    };

    network::request(NetworkRequest::NetworkTest {
        server: "192.168.1.20".to_owned(),
        share: "Photos".to_owned(),
        username: "valerio".to_owned(),
        domain: None,
        password: "the share password".to_owned(),
    })
    .await
    .expect("tried");
    assert!(!machine
        .calls("nft add element inet nas_base nas_smb_servers { 192.168.1.20 }")
        .is_empty());
    assert!(machine
        .calls("mount -t cifs //192.168.1.20/Photos")
        .iter()
        .any(|call| call.contains("credentials=") && call.contains("ip=192.168.1.20")));
    assert!(network::request(NetworkRequest::NetworkTest {
        server: "8.8.8.8".to_owned(),
        share: "Photos".to_owned(),
        username: "valerio".to_owned(),
        domain: None,
        password: "the share password".to_owned(),
    })
    .await
    .expect_err("not local")
    .1
    .contains("not on the local network"));

    network::request(save(Some("the share password")))
        .await
        .expect("saved");
    let listed = network::list().expect("listed");
    let disk = listed.disks.first().expect("one disk");
    assert!(listed.available && !disk.connected && !disk.held);
    let id = disk.definition.id.clone();
    assert!(machine
        .root
        .join(format!("credstore/nas-os.smb-{id}"))
        .is_file());

    assert!(
        network::request(NetworkRequest::NetworkConnect { id: id.clone() })
            .await
            .is_err()
    );
    network::request(NetworkRequest::NetworkDisconnect { id: id.clone() })
        .await
        .expect("disconnected");
    assert!(network::list().expect("listed").disks[0].held);
    network::request(NetworkRequest::NetworkForget { id: id.clone() })
        .await
        .expect("forgotten");
    assert!(network::list().expect("listed").disks.is_empty());
    assert!(!machine
        .root
        .join(format!("credstore/nas-os.smb-{id}"))
        .exists());

    let calls = std::fs::read_to_string(machine.root.join("calls")).expect("calls");
    assert!(!calls.contains("the share password"));
}
