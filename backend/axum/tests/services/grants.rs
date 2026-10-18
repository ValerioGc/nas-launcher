use super::*;

fn asked(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|path| (*path).to_owned()).collect()
}

#[test]
fn grants_only_the_paths_of_the_list_once_each_in_a_fixed_order() {
    let (granted, refused) = split(&asked(&["/sys", "/srv/storage", "/sys", "/dev", "/etc"]));

    assert_eq!(granted, ["/srv/storage", "/sys"]);
    assert_eq!(refused, ["/dev", "/etc"]);
}

#[test]
fn a_package_asking_for_a_path_no_application_is_given_is_refused() {
    assert_eq!(
        checked(&asked(&["/srv/storage", "/proc"])),
        Ok(vec!["/srv/storage", "/proc"])
    );
    assert_eq!(checked(&[]), Ok(Vec::new()));
    assert_eq!(checked(&asked(&["network:smtp"])), Ok(vec!["network:smtp"]));
    assert_eq!(
        checked(&asked(&["run:background", "/srv/storage"])),
        Ok(vec!["/srv/storage", "run:background"])
    );
    for wanted in [
        "network:ftp",
        "run:root",
        "/dev",
        "/srv",
        "/srv/storage/",
        "/var/lib/nas-os",
        "/srv/storage/../etc",
    ] {
        let refused = checked(&asked(&[wanted])).expect_err("refused");
        assert!(refused.contains(wanted), "{refused}");
    }
}

#[test]
fn the_request_holds_one_granted_path_per_line() {
    assert_eq!(
        request_text(&["/srv/storage", "/proc"]),
        "/srv/storage\n/proc\n"
    );
    assert_eq!(request_text(&[]), "");
}

#[tokio::test]
async fn nothing_is_applied_where_the_applications_do_not_run() {
    assert_eq!(
        apply("nas-file-manager", &["/srv/storage"]).await,
        Ok(false)
    );
    assert_eq!(revoke("nas-file-manager").await, Ok(()));
}

#[test]
fn a_request_whose_drop_in_is_missing_is_applied_again() {
    assert!(in_place(false, true));
    assert!(in_place(true, false));
    assert!(
        !in_place(false, false),
        "granted, but the helper wrote nothing"
    );
    assert!(
        !in_place(true, true),
        "nothing granted, but a drop-in is left"
    );
}
