use super::*;
use crate::services::database::open_in_memory;
use axum::http::StatusCode;

const LISTED: &str = r#"{"blockdevices":[
  {"name":"sda","path":"/dev/sda","type":"disk","rm":false,"tran":"sata","children":[
    {"name":"sda1","path":"/dev/sda1","type":"part","fstype":"ext4","uuid":"ROOT-1","mountpoints":["/"]}
  ]},
  {"name":"sdb","path":"/dev/sdb","type":"disk","rm":true,"tran":"usb","children":[
    {"name":"sdb1","path":"/dev/sdb1","type":"part","fstype":"exfat","uuid":"AB12-CD34",
     "mountpoints":["/srv/nas-volumes/v-ab12-cd34","/run/media/stick","/srv/storage/v-ab12-cd34"]},
    {"name":"sdb2","path":"/dev/sdb2","type":"part","fstype":"ext4","uuid":"Data-2","mountpoints":[null]}
  ]},
  {"name":"sdc","path":"/dev/sdc","type":"disk","rm":false,"tran":"sata","children":[
    {"name":"sdc1","path":"/dev/sdc1","type":"part","fstype":"ext4","uuid":"Inner-3","mountpoints":[null]}
  ]}
]}"#;

fn devices() -> Vec<Device> {
    serde_json::from_str::<Devices>(LISTED)
        .expect("listed")
        .blockdevices
}

#[test]
fn a_partition_ejects_the_whole_disk_that_holds_it() {
    let devices = devices();

    assert_eq!(
        disk_of(&devices, "sdb2").and_then(|disk| disk.name.as_deref()),
        Some("sdb")
    );
    assert_eq!(
        disk_of(&devices, "sdb").and_then(|disk| disk.name.as_deref()),
        Some("sdb")
    );
    assert!(disk_of(&devices, "sdz").is_none());
    let stick = disk_of(&devices, "sdb1").expect("stick");
    assert_eq!(uuids(stick), ["ab12-cd34", "data-2"]);
}

#[test]
fn only_an_external_disk_without_the_system_is_ejected() {
    let devices = devices();
    let none = HashSet::new();

    assert!(ejectable(disk_of(&devices, "sdb").expect("stick"), &none).is_ok());
    assert!(ejectable(disk_of(&devices, "sdc").expect("inner"), &none)
        .expect_err("inner disk")
        .contains("external"));
    assert!(ejectable(disk_of(&devices, "sda").expect("system"), &none).is_err());

    let system = HashSet::from(["data-2".to_owned()]);
    assert!(ejectable(disk_of(&devices, "sdb").expect("stick"), &system)
        .expect_err("holds the system")
        .contains("system"));
}

#[test]
fn the_views_of_explorer_come_off_before_the_raw_mounts() {
    let devices = devices();

    assert_eq!(
        mountpoints(disk_of(&devices, "sdb").expect("stick")),
        [
            "/srv/storage/v-ab12-cd34",
            "/srv/nas-volumes/v-ab12-cd34",
            "/run/media/stick"
        ]
    );
}

#[tokio::test]
async fn a_disk_is_marked_by_the_uuid_of_its_filesystem() {
    let db = open_in_memory().await.expect("database");
    let lock = |uuid: &str, locked| LockRequest {
        uuid: uuid.to_owned(),
        locked,
    };

    assert_eq!(
        set_lock(&db, lock("AB12-CD34", true))
            .await
            .expect("locked"),
        ["ab12-cd34"]
    );
    assert_eq!(
        set_lock(&db, lock("ab12-cd34", true)).await.expect("again"),
        ["ab12-cd34"]
    );
    assert!(set_lock(&db, lock("ab12-cd34", false))
        .await
        .expect("unlocked")
        .is_empty());
    for wrong in ["", "../etc", "a b"] {
        let refused = set_lock(&db, lock(wrong, true)).await.expect_err("refused");
        assert_eq!(refused.0, StatusCode::BAD_REQUEST, "{wrong}");
    }
}

#[tokio::test]
async fn an_ejection_needs_a_device_name_and_a_real_installation() {
    let db = open_in_memory().await.expect("database");
    let eject_device = |device: &str| EjectRequest {
        device: device.to_owned(),
    };

    let refused = eject(&db, eject_device("../sdb"))
        .await
        .expect_err("refused");
    assert_eq!(refused.0, StatusCode::BAD_REQUEST);
    let simulated = eject(&db, eject_device("sdb"))
        .await
        .expect_err("simulated");
    assert_eq!(simulated.0, StatusCode::SERVICE_UNAVAILABLE);
}

#[test]
fn the_header_lists_each_external_disk_once_and_keeps_the_volume_locks() {
    let listed = external_inventory(
        serde_json::from_str(LISTED).expect("listed"),
        &["data-2".to_owned()],
        &HashSet::new(),
    );
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].device, "sdb");
    assert!(listed[0].locked);
    assert!(!listed[0].can_eject);

    let free = external_inventory(
        serde_json::from_str(LISTED).expect("listed"),
        &[],
        &HashSet::new(),
    );
    assert!(free[0].can_eject);
}

#[test]
fn unformatted_external_disks_are_listed_but_system_disks_cannot_be_ejected() {
    let listed = r#"{"blockdevices":[
      {"name":"sdb","type":"disk","rm":false,"tran":"usb"},
      {"name":"sdc","type":"disk","tran":"usb","mountpoints":["/"]}
    ]}"#;
    let found = external_inventory(
        serde_json::from_str(listed).expect("listed"),
        &[],
        &HashSet::new(),
    );
    assert_eq!(found.len(), 2);
    assert!(found[0].can_eject);
    assert!(!found[1].can_eject);
}
