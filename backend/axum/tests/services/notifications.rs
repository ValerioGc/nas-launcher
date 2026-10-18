use super::*;
use crate::{
    apps::services::nas_notification_manager::{list, NotificationFilter},
    services::database::open_in_memory,
};

fn from_an_app() -> NewNotification {
    NewNotification {
        title: "Backup failed".to_owned(),
        message: "The disk is full.".to_owned(),
        source: "backup".to_owned(),
        tone: Tone::Error,
        key: None,
        params: BTreeMap::new(),
        log: Some("/var/lib/nas-app/backup/last.log".to_owned()),
    }
}

#[tokio::test]
async fn a_notice_of_the_launcher_keeps_its_key_its_values_and_its_english_text() {
    let db = open_in_memory().await.expect("database");
    record(
        &db,
        "nas-app-manager",
        Tone::Success,
        Notice::new(
            "appInstalled",
            "Application installed",
            "Photos 1.2.0 is installed.",
        )
        .with("app", "Photos")
        .with("version", "1.2.0")
        .log("/var/log/photos.log"),
    )
    .await;
    create(&db, from_an_app()).await.expect("created");

    let found = list(&db, &NotificationFilter::default())
        .await
        .expect("list");
    let installed = &found[1].content;
    assert_eq!(installed.key.as_deref(), Some("appInstalled"));
    assert_eq!(installed.params["version"], "1.2.0");
    assert_eq!(installed.message, "Photos 1.2.0 is installed.");
    assert_eq!(installed.log.as_deref(), Some("/var/log/photos.log"));
    let backup = serde_json::to_value(&found[0]).expect("json");
    assert_eq!(backup["key"], serde_json::Value::Null);
    assert_eq!(backup["log"], "/var/lib/nas-app/backup/last.log");
}

#[test]
fn a_log_is_an_absolute_path_that_climbs_nowhere_and_values_stay_few_and_short() {
    assert!(from_an_app().validate().is_ok());
    for log in ["relative.log", "/var/log/../../etc/shadow", "/var/log/a\nb"] {
        let invalid = NewNotification {
            log: Some(log.to_owned()),
            ..from_an_app()
        };
        assert!(invalid.validate().is_err(), "{log}");
    }
    let many = NewNotification {
        params: (0..13).map(|n| (format!("p{n}"), String::new())).collect(),
        ..from_an_app()
    };
    assert!(many.validate().is_err());
    let long = NewNotification {
        params: BTreeMap::from([("reason".to_owned(), "x".repeat(501))]),
        ..from_an_app()
    };
    assert!(long.validate().is_err());
    let key = NewNotification {
        key: Some("not a key".to_owned()),
        ..from_an_app()
    };
    assert!(key.validate().is_err());
}

#[tokio::test]
async fn the_log_keeps_the_newest_hundred_regardless_of_read_state() {
    let db = open_in_memory().await.expect("database");
    let oldest = create(&db, from_an_app()).await.expect("oldest");
    for _ in 1..100 {
        create(&db, from_an_app()).await.expect("notice");
    }
    let filter = NotificationFilter {
        limit: Some(500),
        ..Default::default()
    };
    assert_eq!(list(&db, &filter).await.unwrap().len(), 100);
    sqlx::query("UPDATE notifications SET read = 1 WHERE id > ?")
        .bind(oldest.id.parse::<i64>().unwrap())
        .execute(&db)
        .await
        .unwrap();
    let newest = create(&db, from_an_app()).await.expect("newest");
    let found = list(&db, &filter).await.unwrap();
    assert_eq!(found.len(), 100);
    assert_eq!(found[0].id, newest.id);
    assert!(found.iter().all(|entry| entry.id != oldest.id));
    assert!(found[1].read);
}

#[tokio::test]
async fn pruning_also_limits_notifications_already_stored_before_startup() {
    let db = open_in_memory().await.expect("database");
    for n in 0..105 {
        sqlx::query(
            "INSERT INTO notifications (source, title, message, tone, read, created_at)
                     VALUES ('backup', ?, '', 'info', 0, ?)",
        )
        .bind(format!("Notice {n}"))
        .bind(n)
        .execute(&db)
        .await
        .unwrap();
    }
    prune(&db).await;
    let found = list(
        &db,
        &NotificationFilter {
            limit: Some(500),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(found.len(), 100);
    assert_eq!(found.first().unwrap().content.title, "Notice 104");
    assert_eq!(found.last().unwrap().content.title, "Notice 5");
}

#[tokio::test]
async fn writes_the_detail_of_a_failure_beside_the_logs_and_links_it() {
    let dir = std::env::temp_dir().join(format!("nas-notice-detail-{}", now_ms()));
    let notice = Notice::new(
        "networkFailed",
        "Network not changed",
        "Unchanged (no device).",
    )
    .detail_in(&dir, "net/../work", "  no device found  ", None)
    .await;
    let path = dir.join("net----work.log");
    assert_eq!(
        notice.log.as_deref(),
        Some(path.display().to_string().as_str())
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("detail"),
        "Network not changed\nUnchanged (no device).\n\nno device found\n"
    );

    let with_journal = Notice::new("wakeFailed", "Not set", "No wake.")
        .detail_in(&dir, "wake", "no rtc", Some("nas-os-test-missing.service"))
        .await;
    let text = std::fs::read_to_string(with_journal.log.expect("linked")).expect("detail");
    assert!(
        text.contains("\nJournal of nas-os-test-missing.service:\n"),
        "{text}"
    );

    let blocked = dir.join("file");
    std::fs::write(&blocked, "").expect("a file in the way");
    let kept = Notice::new("powerRefused", "Refused", "No.")
        .detail_in(&blocked, "power", "denied", None)
        .await;
    assert_eq!(kept.log, None);
    std::fs::remove_dir_all(dir).expect("clean up");
}
