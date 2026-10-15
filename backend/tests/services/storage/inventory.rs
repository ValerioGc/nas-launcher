use super::*;

/// The disk of the system, with LUKS, LVM and a free volume in its group, a data disk with a
/// closed LUKS container and an open one, an empty disk and a USB stick.
const LISTED: &str = r#"{"blockdevices":[
  {"name":"sda","path":"/dev/sda","maj:min":"8:0","type":"disk","size":500,"serial":"SYS-1","wwn":"0x5001","tran":"sata","model":"System SSD","children":[
    {"name":"sda1","path":"/dev/sda1","maj:min":"8:1","type":"part","size":1,"fstype":"vfat","uuid":"EFI","parttype":"c12a7328-f81f-11d2-ba4b-00a0c93ec93b"},
    {"name":"sda2","path":"/dev/sda2","maj:min":"8:2","type":"part","size":499,"fstype":"crypto_LUKS","uuid":"LUKS-SYS","children":[
      {"name":"sda2_crypt","path":"/dev/mapper/sda2_crypt","maj:min":"254:0","type":"crypt","size":499,"fstype":"LVM2_member","children":[
        {"name":"vg-root","path":"/dev/mapper/vg-root","maj:min":"254:1","type":"lvm","size":400,"fstype":"ext4","uuid":"ROOT"},
        {"name":"vg-spare","path":"/dev/mapper/vg-spare","maj:min":"254:2","type":"lvm","size":99,"fstype":"ext4","uuid":"SPARE","label":"Spare"}
      ]}
    ]}
  ]},
  {"name":"sdb","path":"/dev/sdb","maj:min":"8:16","type":"disk","size":2000,"serial":"DATA-1","tran":"sata","children":[
    {"name":"sdb1","path":"/dev/sdb1","maj:min":"8:17","type":"part","size":1000,"fstype":"crypto_LUKS","uuid":"LUKS-CLOSED"},
    {"name":"sdb2","path":"/dev/sdb2","maj:min":"8:18","type":"part","size":1000,"fstype":"crypto_LUKS","uuid":"LUKS-OPEN","children":[
      {"name":"luks-open","path":"/dev/mapper/luks-open","maj:min":"254:3","type":"crypt","size":998,"fstype":"ext4","uuid":"INNER","label":"Vault"}
    ]}
  ]},
  {"name":"sdc","path":"/dev/sdc","maj:min":"8:32","type":"disk","size":3000,"serial":"EMPTY-1","tran":"sata"},
  {"name":"sdd","path":"/dev/sdd","maj:min":"8:48","type":"disk","size":64,"rm":true,"tran":"usb","children":[
    {"name":"sdd1","path":"/dev/sdd1","maj:min":"8:49","type":"part","size":64,"fstype":"exfat","uuid":"STICK","label":"Stick"}
  ]}
]}"#;

/// The mounts of the host: the root, and the open container under a path with a space.
const MOUNTINFO: &str = "\
22 1 254:1 / / rw,relatime shared:1 - ext4 /dev/mapper/vg-root rw
40 22 254:3 / /srv/nas-volumes/v-inner rw,nosuid shared:2 - ext4 /dev/mapper/luks-open rw
41 22 254:3 /.nas-users/valerio /srv/storage/My\\040Vault rw shared:3 - ext4 /dev/mapper/luks-open rw
";

fn listed() -> Vec<Disk> {
    let devices = serde_json::from_str::<Devices>(LISTED).expect("listed");
    inventory(devices, MOUNTINFO, &HashSet::new())
}

fn entry<'a>(disks: &'a [Disk], id: &str) -> Option<&'a Disk> {
    disks.iter().find(|disk| disk.id == id)
}

#[test]
fn the_system_is_left_out_and_its_disk_is_never_erased() {
    let disks = listed();

    for hidden in ["sda", "sda1", "sda2", "sda2_crypt", "vg-root"] {
        assert!(entry(&disks, hidden).is_none(), "{hidden} is listed");
    }
    let spare = entry(&disks, "vg-spare").expect("the free volume of the group");
    assert!(spare.system_disk);
    assert!(!spare.initializable);
    assert_eq!(spare.name, "Spare");
    assert!(disks
        .iter()
        .filter(|disk| disk.id != "vg-spare")
        .all(|disk| !disk.system_disk));
}

#[test]
fn a_luks_container_says_whether_it_is_open() {
    let disks = listed();

    let closed = entry(&disks, "sdb1").expect("closed container");
    assert_eq!(closed.encryption, Some("locked"));
    assert_eq!(closed.filesystem.as_deref(), Some("crypto_LUKS"));
    let open = entry(&disks, "sdb2").expect("open container");
    assert_eq!(open.encryption, Some("open"));
    let inner = entry(&disks, "luks-open").expect("its filesystem");
    assert_eq!(inner.encryption, None);
    assert_eq!(inner.parent.as_deref(), Some("sdb2"));
    assert_eq!(inner.mount.as_deref(), Some("/srv/nas-volumes/v-inner"));
}

#[test]
fn every_volume_carries_the_identity_of_its_whole_disk() {
    let disks = listed();

    assert_eq!(
        entry(&disks, "sdb1").expect("sdb1").identity,
        "/DATA-1/2000"
    );
    assert_eq!(
        entry(&disks, "luks-open").expect("inner").identity,
        "/DATA-1/2000"
    );
    assert_eq!(
        entry(&disks, "vg-spare").expect("spare").identity,
        "0x5001/SYS-1/500"
    );
}

#[test]
fn only_an_empty_data_disk_can_be_initialized() {
    let disks = listed();

    let empty = entry(&disks, "sdc").expect("the empty disk");
    assert!(empty.initializable);
    assert_eq!(empty.kind, "local");
    assert!(entry(&disks, "sdb").is_none(), "a disk with partitions");
    assert!(disks
        .iter()
        .filter(|disk| disk.id != "sdc")
        .all(|disk| !disk.initializable));
}

#[test]
fn a_removable_or_usb_disk_is_external() {
    let disks = listed();

    let stick = entry(&disks, "sdd1").expect("the stick");
    assert_eq!(stick.kind, "external");
    assert_eq!(stick.transport.as_deref(), Some("usb"));
    assert_eq!(stick.mount, None);
}

#[test]
fn a_filesystem_fstab_gives_the_system_is_left_out_even_unmounted() {
    let devices = serde_json::from_str::<Devices>(LISTED).expect("listed");
    let system =
        fstab_system_uuids("# UUID=STICK /home ext4\nUUID=stick /home exfat defaults 0 2\n");
    let disks = inventory(devices, MOUNTINFO, &system);

    assert!(entry(&disks, "sdd1").is_none());
    assert!(entry(&disks, "sdd").is_none());
}

#[test]
fn fstab_lines_are_read_without_comments() {
    let fstab = "# LABEL=Old /mnt/old ext4\nLABEL=Media /mnt/media ext4 defaults 0 2\nUUID=ABC / ext4 defaults 0 1\nUUID=DEF /mnt/data ext4 defaults 0 2\n";

    assert_eq!(fstab_labels(fstab), HashSet::from(["Media".to_owned()]));
    assert_eq!(fstab_system_uuids(fstab), HashSet::from(["abc".to_owned()]));
}

#[test]
fn an_identity_is_short_and_has_no_control_character() {
    assert!(valid_identity("0x5001/SYS-1/500"));
    assert!(valid_identity("//0"));
    assert!(!valid_identity(""));
    assert!(!valid_identity("a\tb"));
    assert!(!valid_identity(&"x".repeat(257)));
}

#[test]
fn find_reaches_a_block_deep_in_the_tree() {
    let devices = serde_json::from_str::<Devices>(LISTED).expect("listed");

    let found = find(&devices.blockdevices, "vg-spare").expect("found");
    assert_eq!(found.uuid.as_deref(), Some("SPARE"));
    assert!(find(&devices.blockdevices, "sdz").is_none());
}

fn parsed(json: &str, mountinfo: &str) -> Vec<Disk> {
    let devices = serde_json::from_str::<Devices>(json).expect("listed");
    inventory(devices, mountinfo, &HashSet::new())
}

#[test]
fn a_data_partition_on_the_disk_of_the_system_is_listed_but_not_its_empty_ones() {
    let disks = parsed(
        r#"{"blockdevices":[{"name":"sda","type":"disk","size":1000},{"name":"sdb","type":"disk","size":2000,"children":[
            {"name":"sdb1","type":"part","fstype":"ext4","mountpoints":["/"]},
            {"name":"sdb2","type":"part"},
            {"name":"sdb3","type":"part","fstype":"ext4","mountpoints":["/srv/storage/v-data"]},
            {"name":"sdb5","type":"part","fstype":"swap","mountpoints":["[SWAP]"]}
        ]}]}"#,
        "",
    );

    assert_eq!(
        disks
            .iter()
            .map(|disk| disk.id.as_str())
            .collect::<Vec<_>>(),
        ["sda", "sdb3"]
    );
    assert!(disks[0].initializable);
    assert!(!disks[0].system_disk);
    assert!(disks[1].system_disk);
}

#[test]
fn the_host_mounts_replace_the_ones_lsblk_sees() {
    let disks = parsed(
        r#"{"blockdevices":[
            {"name":"sda","type":"disk","maj:min":"8:0","children":[
                {"name":"sda1","type":"part","maj:min":"8:1","fstype":"ext4","mountpoints":[null]},
                {"name":"sda2","type":"part","maj:min":"8:2","fstype":"ext4","mountpoints":["/stale"]}
            ]},
            {"name":"sdc1","type":"part","fstype":"ext4","mountpoints":[null," ","/srv/storage/data","/mnt/data"]}
        ]}"#,
        "30 25 8:1 / / rw - ext4 /dev/sda1 rw\n",
    );

    assert_eq!(
        disks
            .iter()
            .map(|disk| disk.id.as_str())
            .collect::<Vec<_>>(),
        ["sda2", "sdc1"]
    );
    assert_eq!(disks[0].mount, None);
    assert!(disks[0].system_disk);
    assert_eq!(disks[1].mount.as_deref(), Some("/srv/storage/data"));
}

#[test]
fn the_mountpoints_of_a_mountinfo_are_read_unescaped() {
    let found = mountpoints(MOUNTINFO);
    assert!(found.contains(&PathBuf::from("/srv/storage/My Vault")));
    assert!(found.contains(&PathBuf::from("/")));
    assert_eq!(found.len(), 3);
}

#[test]
fn a_backslash_that_starts_no_octal_escape_is_kept() {
    assert_eq!(unescape_mount(r"/mnt/my\040disk"), "/mnt/my disk");
    assert_eq!(unescape_mount(r"/mnt/caf\303\251"), "/mnt/café");
    assert_eq!(unescape_mount(r"/mnt/a\xyzb"), r"/mnt/a\xyzb");
    assert_eq!(unescape_mount(r"/mnt/end\0"), r"/mnt/end\0");
}
