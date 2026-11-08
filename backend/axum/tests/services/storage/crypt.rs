use super::*;

fn format(encrypt: bool, automatic: bool, password: Option<&str>) -> Request {
    Request::Format {
        device: "sdb".to_owned(),
        identity: "wwn/S1/1024".to_owned(),
        label: "Data".to_owned(),
        encrypt,
        automatic,
        password: password.map(str::to_owned),
        current: None,
    }
}

#[test]
fn a_format_asks_for_keys_only_when_it_encrypts() {
    assert!(validate(&format(false, false, None)).is_ok());
    assert!(validate(&format(true, true, None)).is_ok());
    assert!(validate(&format(true, false, Some("long enough"))).is_ok());
    assert!(validate(&format(true, false, None)).is_err());
    assert!(validate(&format(false, true, None)).is_err());
    assert!(validate(&format(false, false, Some("long enough"))).is_err());
    assert!(validate(&format(true, false, Some("short"))).is_err());
    assert!(validate(&format(true, false, Some("line\nbreak"))).is_err());
}

#[test]
fn a_format_names_a_disk_its_identity_and_a_label() {
    let with = |device: &str, identity: &str, label: &str| Request::Format {
        device: device.to_owned(),
        identity: identity.to_owned(),
        label: label.to_owned(),
        encrypt: false,
        automatic: false,
        password: None,
        current: None,
    };
    assert!(validate(&with("../sda", "x/y/1", "Data")).is_err());
    assert!(validate(&with("sdb", "", "Data")).is_err());
    assert!(validate(&with("sdb", "x/y/1", "bad/label")).is_err());
    assert!(validate(&with("sdb", "x/y/1", " Data")).is_err());
}

#[test]
fn the_other_requests_name_a_volume_and_bring_valid_secrets() {
    let uuid = "0f3c-ab".to_owned();
    for valid in [
        Request::Unlock {
            uuid: uuid.clone(),
            password: Some("long enough".to_owned()),
        },
        Request::Unlock {
            uuid: uuid.clone(),
            password: None,
        },
        Request::Lock { uuid: uuid.clone() },
        Request::Automatic {
            uuid: uuid.clone(),
            enabled: true,
            password: Some("long enough".to_owned()),
        },
        Request::Automatic {
            uuid: uuid.clone(),
            enabled: false,
            password: None,
        },
        Request::Password {
            uuid: uuid.clone(),
            current: None,
            password: "long enough".to_owned(),
        },
        Request::Recovery {
            uuid: uuid.clone(),
            current: None,
        },
        Request::Header { uuid: uuid.clone() },
    ] {
        assert!(validate(&valid).is_ok(), "{valid:?}");
    }
    for invalid in [
        Request::Unlock {
            uuid: uuid.clone(),
            password: Some("short".to_owned()),
        },
        Request::Lock {
            uuid: "../sda".to_owned(),
        },
        Request::Automatic {
            uuid: uuid.clone(),
            enabled: true,
            password: None,
        },
        Request::Password {
            uuid: uuid.clone(),
            current: Some("tab\there".to_owned()),
            password: "long enough".to_owned(),
        },
        Request::Recovery {
            uuid: String::new(),
            current: None,
        },
    ] {
        assert!(validate(&invalid).is_err(), "{invalid:?}");
    }
}

#[test]
fn a_request_reads_with_its_operation_and_refuses_unknown_fields() {
    let request: Request =
        serde_json::from_str(r#"{"op":"unlock","uuid":"ab-12","password":"long enough"}"#)
            .expect("request");
    assert!(matches!(request, Request::Unlock { .. }));
    let format: Request = serde_json::from_str(
        r#"{"op":"format","device":"sdb","identity":"x/y/1","label":"Data","encrypt":true,"automatic":true}"#,
    )
    .expect("format");
    assert!(matches!(
        format,
        Request::Format {
            automatic: true,
            password: None,
            ..
        }
    ));
    assert!(serde_json::from_str::<Request>(r#"{"op":"lock","uuid":"ab","extra":1}"#).is_err());
    assert!(serde_json::from_str::<Request>(r#"{"op":"wipe","uuid":"ab"}"#).is_err());
}

#[test]
fn an_answer_says_only_what_it_carries() {
    assert_eq!(
        serde_json::to_string(&Answer::default()).expect("json"),
        "{}"
    );
    let recovery = Answer {
        recovery: Some("key".to_owned()),
        ..Answer::default()
    };
    assert_eq!(
        serde_json::to_string(&recovery).expect("json"),
        r#"{"recovery":"key"}"#
    );
}

#[tokio::test]
async fn nothing_is_asked_while_encryption_is_off() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("database");
    let refused = request(
        &db,
        Request::Lock {
            uuid: "ab".to_owned(),
        },
    )
    .await;
    let invalid = request(
        &db,
        Request::Lock {
            uuid: "a b".to_owned(),
        },
    )
    .await;

    if !available() {
        assert!(refused.is_err());
    }
    assert!(invalid.is_err());
}

#[tokio::test]
async fn a_volume_with_the_automatic_key_alone_is_reported_until_read() {
    let db = crate::services::database::open_in_memory()
        .await
        .expect("database");
    let directory = std::env::temp_dir().join(format!("nas-crypt-keys-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("directory");
    std::env::set_var("NAS_STORAGE_STATE_DIR", &directory);
    let warnings = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM notifications WHERE key = 'diskWithoutSpareKey'",
        )
        .fetch_one(&db)
        .await
        .expect("notifications")
    };
    let write = |keys: &str| std::fs::write(status_path(), keys).expect("status");

    write(r#"{"a":{"password":true,"automatic":true,"recovery":false,"held":false}}"#);
    report_spare_keys(&db).await;
    assert_eq!(warnings().await, 0);

    write(r#"{"a":{"password":false,"automatic":true,"recovery":false,"held":false}}"#);
    report_spare_keys(&db).await;
    report_spare_keys(&db).await;
    assert_eq!(warnings().await, 1);
    std::env::remove_var("NAS_STORAGE_STATE_DIR");
    std::fs::remove_dir_all(&directory).expect("removed");
}

#[test]
fn a_password_or_a_key_only_goes_with_a_request_that_says_so() {
    let uuid = || "0f8fad5b-d9cb-469f-a165-70867728950e".to_owned();
    let format = |password: Option<&str>| Request::Format {
        device: "sdb".to_owned(),
        identity: "wwn/1".to_owned(),
        label: "Dati".to_owned(),
        encrypt: true,
        automatic: true,
        password: password.map(str::to_owned),
        current: None,
    };
    assert!(!format(None).carries_secret());
    assert!(format(Some("long enough")).carries_secret());
    let erasing = |current: &str| Request::Format {
        device: "sdb".to_owned(),
        identity: "wwn/1".to_owned(),
        label: "Dati".to_owned(),
        encrypt: false,
        automatic: false,
        password: None,
        current: Some(current.to_owned()),
    };
    assert!(erasing("long enough").carries_secret());
    assert!(validate(&erasing("long enough")).is_ok());
    assert!(validate(&erasing("short")).is_err());
    assert!(!Request::Lock { uuid: uuid() }.carries_secret());
    assert!(Request::Header { uuid: uuid() }.carries_secret());
    assert!(Request::Recovery {
        uuid: uuid(),
        current: None
    }
    .carries_secret());
    assert!(Request::Unlock {
        uuid: uuid(),
        password: Some("long enough".to_owned())
    }
    .carries_secret());
    assert!(!Request::Unlock {
        uuid: uuid(),
        password: None
    }
    .carries_secret());
    assert!(!Request::Automatic {
        uuid: uuid(),
        enabled: false,
        password: None
    }
    .carries_secret());
}

fn listed(json: &str) -> inventory::Device {
    serde_json::from_str::<inventory::Devices>(json)
        .expect("listed")
        .blockdevices
        .remove(0)
}

const LOCKED_DISK: &str = r#"{"blockdevices":[{"name":"sdb","path":"/dev/sdb","type":"disk","children":[
    {"name":"sdb1","path":"/dev/sdb1","type":"part","fstype":"crypto_LUKS","uuid":"AA-LOCKED"},
    {"name":"sdb2","path":"/dev/sdb2","type":"part","fstype":"crypto_LUKS","uuid":"BB-OPEN","children":[
        {"name":"nas-luks-bb-open","path":"/dev/mapper/nas-luks-bb-open","type":"crypt","fstype":"ext4"}
    ]}
]}]}"#;

#[test]
fn only_the_closed_volumes_of_a_disk_count_as_locked() {
    let disk = listed(LOCKED_DISK);
    let closed: Vec<_> = locked(&disk)
        .iter()
        .filter_map(|container| container.uuid.clone())
        .collect();
    assert_eq!(closed, ["AA-LOCKED"]);

    let plain = listed(
        r#"{"blockdevices":[{"name":"sdc","path":"/dev/sdc","type":"disk","children":[
            {"name":"sdc1","path":"/dev/sdc1","type":"part","fstype":"ext4"}
        ]}]}"#,
    );
    assert!(locked(&plain).is_empty());
}

#[test]
fn a_locked_volume_asks_for_its_key_unless_the_automatic_key_is_all_it_has() {
    let disk = listed(LOCKED_DISK);
    let locked = locked(&disk);
    let with = |keys: Keys| HashMap::from([("aa-locked".to_owned(), keys)]);
    let automatic = Keys {
        automatic: true,
        held: true,
        ..Keys::default()
    };

    assert!(needs_key(&locked, &HashMap::new()));
    assert!(needs_key(
        &locked,
        &with(Keys {
            password: true,
            ..automatic
        })
    ));
    assert!(needs_key(
        &locked,
        &with(Keys {
            recovery: true,
            ..automatic
        })
    ));
    assert!(needs_key(&locked, &with(Keys::default())));
    assert!(!needs_key(&locked, &with(automatic)));
    assert!(!needs_key(&[], &HashMap::new()));
}
