use super::*;

fn notification(tone: Tone, message: &str, params: &[(&str, &str)]) -> Notification {
    Notification {
        id: "1".to_owned(),
        content: crate::services::notifications::NewNotification {
            title: "Event".to_owned(),
            message: message.to_owned(),
            source: "nas-app-manager".to_owned(),
            tone,
            key: Some("taskFailed".to_owned()),
            params: params
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
            log: None,
        },
        at: 1,
        read: false,
    }
}

#[tokio::test]
async fn only_warnings_and_errors_with_a_real_log_can_be_opened() {
    let missing = read(&notification(
        Tone::Error,
        "Installation failed",
        &[("reason", "archive has no manifest")],
    ))
    .await
    .unwrap_err();
    assert_eq!(missing.0.as_u16(), 404);

    let refused = read(&notification(Tone::Success, "Installed", &[]))
        .await
        .unwrap_err();
    assert_eq!(refused.0.as_u16(), 400);
}

#[tokio::test]
async fn a_log_file_is_read_from_its_end_and_only_under_its_root() {
    let base = std::env::temp_dir().join(format!("nas-os-logs-{}", std::process::id()));
    let root = base.join("log");
    std::fs::create_dir_all(&root).expect("root");
    let written: String = (1..=1500).map(|n| format!("line {n}\n")).collect();
    std::fs::write(root.join("backup.log"), &written).expect("log");
    std::fs::write(base.join("secret.db"), "secret").expect("secret");
    std::os::unix::fs::symlink(base.join("secret.db"), root.join("link.log")).expect("link");
    let root = std::fs::canonicalize(&root).expect("resolved root");

    let log = file(&root, root.join("backup.log").to_str().expect("path"))
        .await
        .expect("read");
    assert_eq!(log.kind, "file");
    assert_eq!(log.lines.len(), MAX_LINES);
    assert_eq!(log.lines.last().map(String::as_str), Some("line 1500"));

    for outside in [root.join("link.log"), base.join("secret.db")] {
        let refused = file(&root, outside.to_str().expect("path"))
            .await
            .unwrap_err();
        assert_eq!(refused.0.as_u16(), 400);
    }
    let missing = file(&root, root.join("none.log").to_str().expect("path"))
        .await
        .unwrap_err();
    assert_eq!(missing.0.as_u16(), 503);
    std::fs::remove_dir_all(&base).expect("cleanup");
}
