use super::*;
use crate::services::manifest::Manifest;

fn share(name: &str, path: &str) -> Share {
    Share {
        name: name.to_owned(),
        path: path.to_owned(),
        writable: false,
        protected: true,
        trash: None,
    }
}

fn manifest(paths: &[&str], shares: bool) -> Manifest {
    Manifest {
        id: "nas-file-manager".to_owned(),
        name: "Explorer".to_owned(),
        system: true,
        version: "1.0.0".to_owned(),
        icon: String::new(),
        category: None,
        protocol: 1,
        provides: Vec::new(),
        paths: paths.iter().map(|path| (*path).to_owned()).collect(),
        network: Vec::new(),
        packages: Vec::new(),
        background: false,
        shares,
        cleanup: false,
        socket: PathBuf::new(),
        exec: String::new(),
    }
}

#[test]
fn a_share_is_named_as_windows_and_samba_both_read_it() {
    for name in ["Foto", "Foto 2026", "documenti_casa", "a.b-c", "Città"] {
        assert!(is_name(name), "{name}");
    }
    for name in [
        "",
        " Foto",
        "Foto ",
        "a]b",
        "a/b",
        "a%u",
        "tab\tname",
        "global",
        "Homes",
        "IPC$",
        &"x".repeat(65),
    ] {
        assert!(!is_name(name), "{name:?}");
    }
}

#[test]
fn a_share_is_a_folder_below_the_data_disks_written_plainly() {
    for path in [
        "/srv/storage/disk/Foto",
        "/srv/storage/disk",
        "/srv/storage/d/a b/c",
    ] {
        assert!(is_path(path), "{path}");
    }
    for path in [
        "/srv/storage",
        "/srv/storage/",
        "/srv/storagex/a",
        "/srv/storage/../etc",
        "/srv/storage/a/./b",
        "/srv/storage/a//b",
        "/srv/storage/a/",
        "/srv/storage/a%u",
        "/srv/storage/d/ spaced",
        "/srv/storage/d/spaced /x",
        "/srv/storage/a\nb",
        "/etc",
        "srv/storage/a",
    ] {
        assert!(!is_path(path), "{path:?}");
    }
}

#[test]
fn a_list_shares_each_name_and_each_folder_once() {
    assert!(checked(&[share("Foto", "/srv/storage/d/Foto")]).is_ok());
    assert!(checked(&[
        share("Foto", "/srv/storage/d/Foto"),
        share("foto", "/srv/storage/d/Altro"),
    ])
    .is_err());
    assert!(checked(&[
        share("Foto", "/srv/storage/d/Foto"),
        share("Altro", "/srv/storage/d/Foto"),
    ])
    .is_err());
    assert!(checked(&[share("Foto", "/etc")]).is_err());
    assert!(checked(&[share("global", "/srv/storage/d/Foto")]).is_err());
    let many: Vec<Share> = (0..=MAX_SHARES)
        .map(|n| share(&format!("s{n}"), &format!("/srv/storage/d/{n}")))
        .collect();
    assert!(checked(&many).is_err());
}

#[test]
fn the_request_has_one_line_for_each_share() {
    let mut open = share("Musica", "/srv/storage/d/Musica");
    open.writable = true;
    open.protected = false;
    open.trash = Some("/srv/storage/d/.nas-trash/Musica".to_owned());
    assert_eq!(
        request_text(&[share("Foto", "/srv/storage/d/Foto"), open]),
        "Foto\t/srv/storage/d/Foto\tro\tprotected\t-\n\
         Musica\t/srv/storage/d/Musica\trw\topen\t/srv/storage/d/.nas-trash/Musica\n"
    );
    assert_eq!(request_text(&[]), "");
}

#[test]
fn a_share_sends_its_deleted_files_only_to_the_trash_of_its_own_disk() {
    for (trash, path) in [
        ("/srv/storage/d/.nas-trash/Foto", "/srv/storage/d/Foto"),
        ("/srv/storage/d/.nas-trash/a/b", "/srv/storage/d/a/b"),
        ("/srv/storage/d/.nas-trash", "/srv/storage/d"),
    ] {
        assert!(is_trash_of(trash, path), "{trash}");
    }
    for (trash, path) in [
        ("/srv/storage/d/.nas-trash/Foto", "/srv/storage/e/Foto"),
        ("/srv/storage/d/.nas-trashes/Foto", "/srv/storage/d/Foto"),
        (
            "/srv/storage/d/.nas-trash/.nas-trash/Foto",
            "/srv/storage/d/.nas-trash/Foto",
        ),
        ("/srv/storage/d/trash/Foto", "/srv/storage/d/Foto"),
        ("/etc/.nas-trash/Foto", "/etc/Foto"),
    ] {
        assert!(!is_trash_of(trash, path), "{trash}");
    }
    let mut wrong = share("Foto", "/srv/storage/d/Foto");
    wrong.trash = Some("/srv/storage/x/.nas-trash/Foto".to_owned());
    assert!(checked(&[wrong]).is_err());
}

#[test]
fn only_an_application_with_the_data_disks_that_asks_may_share() {
    assert!(may_share(&manifest(&["/srv/storage"], true)));
    assert!(!may_share(&manifest(&["/srv/storage"], false)));
    assert!(!may_share(&manifest(&["/proc"], true)));
}

#[tokio::test]
async fn an_application_that_does_not_declare_it_shares_nothing() {
    let refused = declare("nas-not-installed", &[share("Foto", "/srv/storage/d/Foto")])
        .await
        .expect_err("refused");
    assert_eq!(refused.0, axum::http::StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn sharing_says_whether_samba_is_there() {
    unsafe { std::env::set_var("NAS_SAMBA_SMBD", "/nonexistent/smbd") };
    assert!(!sharing().await.available);
    unsafe { std::env::set_var("NAS_SAMBA_SMBD", "/bin/sh") };
    assert!(sharing().await.available);
    assert!(sharing().await.recycle);
}
