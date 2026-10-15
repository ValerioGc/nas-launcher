use super::*;

fn save(id: Option<&str>, password: Option<&str>) -> NetworkRequest {
    NetworkRequest::NetworkSave {
        id: id.map(str::to_owned),
        name: "Office NAS".to_owned(),
        server: "192.168.1.20".to_owned(),
        share: "Photos 2026".to_owned(),
        username: "valerio".to_owned(),
        domain: Some("WORKGROUP".to_owned()),
        password: password.map(str::to_owned),
        automatic: true,
    }
}

#[test]
fn a_new_network_disk_brings_its_password_a_change_may_keep_it() {
    assert!(validate(&save(None, Some("secret"))).is_ok());
    assert!(validate(&save(None, None)).is_err());
    assert!(validate(&save(Some("n-0a1b2c3d"), None)).is_ok());
    assert!(validate(&save(Some("v-0a1b2c3d"), None)).is_err());
    assert!(validate(&save(None, Some("line\nbreak"))).is_err());
}

#[test]
fn server_share_user_and_name_are_what_a_mount_and_the_registry_take() {
    let with =
        |server: &str, share: &str, username: &str, name: &str| NetworkRequest::NetworkSave {
            id: None,
            name: name.to_owned(),
            server: server.to_owned(),
            share: share.to_owned(),
            username: username.to_owned(),
            domain: None,
            password: Some("secret".to_owned()),
            automatic: false,
        };
    for (server, share, username, name) in [
        ("-o", "share", "user", "Name"),
        ("nas.local/../x", "share", "user", "Name"),
        ("nas.local", "a/b", "user", "Name"),
        ("nas.local", "a\\b", "user", "Name"),
        ("nas.local", "share", "", "Name"),
        ("nas.local", "share", "user", "tab\there"),
        ("nas.local", "share", "user", " Name"),
    ] {
        assert!(
            validate(&with(server, share, username, name)).is_err(),
            "{server} {share} {username} {name}"
        );
    }
    assert!(validate(&with("fe80::1", "share", "user", "Name")).is_ok());
    assert!(validate(&NetworkRequest::NetworkConnect {
        id: "n-0a1b2c3d".to_owned()
    })
    .is_ok());
    assert!(validate(&NetworkRequest::NetworkForget {
        id: "../x".to_owned()
    })
    .is_err());
    assert!(validate(&NetworkRequest::NetworkTest {
        server: "nas.local".to_owned(),
        share: "share".to_owned(),
        username: "user".to_owned(),
        domain: Some("bad domain".to_owned()),
        password: "secret".to_owned(),
    })
    .is_err());
}

#[test]
fn a_request_reads_with_its_operation() {
    let request: NetworkRequest =
        serde_json::from_str(r#"{"op":"networkDisconnect","id":"n-0a1b2c3d"}"#).expect("request");
    assert!(matches!(request, NetworkRequest::NetworkDisconnect { .. }));
    assert!(serde_json::from_str::<NetworkRequest>(r#"{"op":"lock","uuid":"x"}"#).is_err());
}

#[tokio::test]
async fn nothing_is_asked_while_network_disks_are_off() {
    if !available() {
        assert!(request(NetworkRequest::NetworkConnect {
            id: "n-0a1b2c3d".to_owned()
        })
        .await
        .is_err());
    }
    assert!(
        request(NetworkRequest::NetworkConnect { id: "x".to_owned() })
            .await
            .is_err()
    );
}
