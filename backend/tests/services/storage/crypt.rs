use super::*;

fn format(encrypt: bool, automatic: bool, password: Option<&str>) -> Request {
    Request::Format {
        device: "sdb".to_owned(),
        identity: "wwn/S1/1024".to_owned(),
        label: "Data".to_owned(),
        encrypt,
        automatic,
        password: password.map(str::to_owned),
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
            password: "long enough".to_owned(),
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
            password: "short".to_owned(),
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
