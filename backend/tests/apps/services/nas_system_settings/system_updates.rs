use super::*;

#[test]
fn the_upgrades_are_read_from_what_apt_lists() {
    let output = "Listing...\n\
        bash/stable 5.2.37-2 amd64 [upgradable from: 5.2.37-1]\n\
        libc6/stable-security 2.41-12+deb13u1 amd64 [upgradable from: 2.41-12]\n";

    assert_eq!(
        parse(output),
        vec![
            Package {
                id: "bash".into(),
                current: "5.2.37-1".into(),
                version: "5.2.37-2".into(),
                level: Level::Stable,
            },
            Package {
                id: "libc6".into(),
                current: "2.41-12".into(),
                version: "2.41-12+deb13u1".into(),
                level: Level::Security,
            },
        ]
    );
}

#[test]
fn an_upgrade_is_as_important_as_the_archive_it_comes_from() {
    assert_eq!(Level::of("trixie-security"), Level::Security);
    assert_eq!(Level::of("trixie-updates,trixie-security"), Level::Security);
    assert_eq!(Level::of("trixie"), Level::Stable);
    assert_eq!(Level::of("stable-updates"), Level::Stable);
    assert_eq!(Level::of("trixie-proposed-updates"), Level::Stable);
    assert_eq!(Level::of("trixie-backports"), Level::Other);
}

#[test]
fn a_line_that_is_not_an_upgrade_is_left_out() {
    assert!(parse("Listing...\nWARNING: apt does not have a stable CLI interface.\n").is_empty());
}

#[test]
fn only_a_debian_package_name_is_one() {
    for name in ["bash", "libc6", "g++-14", "python3.13", "0ad"] {
        assert!(is_package_name(name), "{name}");
    }
    for name in [
        "",
        "Bash",
        "-rf",
        "a b",
        "bash;reboot",
        "../etc",
        &"a".repeat(129),
    ] {
        assert!(!is_package_name(name), "{name}");
    }
}

#[tokio::test]
async fn nothing_is_run_where_the_mode_is_not_real() {
    let db = database::open_in_memory()
        .await
        .expect("in-memory database");
    let session = Session::default();

    let refused = start(&db, &session, Job::Refresh, &[]).await;

    assert!(refused.is_err_and(|failure| failure.0 == StatusCode::SERVICE_UNAVAILABLE));
    assert!(session.0.read().await.apps.apt_job.is_none());
}
