use super::*;

#[test]
fn only_the_addresses_of_the_local_network_are_opened() {
    for local in [
        "192.168.1.20",
        "10.0.0.5",
        "172.20.1.1",
        "169.254.3.4",
        "fd00::1",
        "fe80::2",
    ] {
        assert!(is_local(local.parse().expect("address")), "{local}");
    }
    for public in ["8.8.8.8", "172.32.0.1", "2001:db8::1", "127.0.0.1", "::1"] {
        assert!(!is_local(public.parse().expect("address")), "{public}");
    }
}

#[test]
fn the_addresses_of_a_server_come_from_getent_once_each() {
    let listing =
        "192.168.1.20    STREAM nas.local\n192.168.1.20    DGRAM\nfe80::2 STREAM\nnot an address\n";
    assert_eq!(
        addresses_of(listing),
        [
            "192.168.1.20".parse::<IpAddr>().expect("v4"),
            "fe80::2".parse().expect("v6")
        ]
    );
}

#[test]
fn the_mount_keeps_the_password_in_its_file_and_the_share_to_its_owners() {
    let arguments = mount_arguments(
        "nas.local",
        "Photos 2026",
        "192.168.1.20".parse().expect("address"),
        "/run/keys/credentials",
        ("1000", "998"),
        Path::new("/srv/storage/n-0a1b2c3d"),
    );

    assert_eq!(arguments[2], "//nas.local/Photos 2026");
    assert_eq!(arguments[3], "/srv/storage/n-0a1b2c3d");
    assert!(arguments[5]
        .starts_with("credentials=/run/keys/credentials,ip=192.168.1.20,uid=1000,gid=998"));
    assert!(arguments[5].contains("soft"));
    assert!(arguments[5].contains("nosuid,nodev,noexec"));
    assert!(!arguments.join(" ").contains("password"));
    assert_eq!(
        credentials_of("valerio", "p,a=ss", Some("WORKGROUP")),
        "username=valerio\npassword=p,a=ss\ndomain=WORKGROUP\n"
    );
    assert_eq!(
        credentials_of("valerio", "x", None),
        "username=valerio\npassword=x\n"
    );
}

#[test]
fn a_reconciliation_out_of_time_tries_no_network_disk() {
    let pending = [Definition {
        id: "n-0a1b2c3d".to_owned(),
        name: "Office".to_owned(),
        server: "192.168.1.20".to_owned(),
        share: "Photos".to_owned(),
        username: "valerio".to_owned(),
        domain: None,
        automatic: true,
    }];

    assert!(connect_pending(&pending, ("1000", "998"), std::time::Duration::ZERO).is_empty());
    assert!(connect_pending(&[], ("1000", "998"), std::time::Duration::from_secs(1)).is_empty());
}

#[test]
fn the_shares_still_to_mount_are_those_that_connect_by_themselves() {
    let directory = std::env::temp_dir().join(format!("nas-network-root-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("directory");
    let list = directory.join("network-disks.json");
    let disk = |id: &str, automatic: bool| Definition {
        id: id.to_owned(),
        name: id.to_owned(),
        server: "192.168.1.20".to_owned(),
        share: "Photos".to_owned(),
        username: "valerio".to_owned(),
        domain: None,
        automatic,
    };
    std::env::set_var("NAS_NETWORK_DISKS", &list);
    save_definitions(&[disk("n-0a1b2c3d", true), disk("n-0a1b2c3e", false)]).expect("saved");

    let (grants, pending) = mounted_shares();
    assert!(grants.is_empty());
    assert_eq!(pending, [disk("n-0a1b2c3d", true)]);
    // Without its password the share is not mounted, and the reconciliation goes on.
    assert!(
        connect_pending(&pending, ("1000", "998"), std::time::Duration::from_secs(5)).is_empty()
    );
    assert_eq!(grant(&pending[0]).kind, "network");
    std::env::remove_var("NAS_NETWORK_DISKS");
    std::fs::remove_dir_all(&directory).expect("removed");
}
