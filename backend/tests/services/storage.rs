use super::*;

fn classify_with_system_uuids(
    json: &str,
    protected_uuids: &HashSet<String>,
) -> Result<Vec<Candidate>, String> {
    let devices: Devices = serde_json::from_str(json).map_err(|error| error.to_string())?;
    Ok(inventory::classify(devices, protected_uuids))
}

/// The disks of `json`, with no filesystem of the system known from fstab.
fn classify(json: &str) -> Result<Vec<Candidate>, String> {
    classify_with_system_uuids(json, &HashSet::new())
}

fn listed(json: &str) -> Vec<Device> {
    serde_json::from_str::<Devices>(json)
        .expect("listed")
        .blockdevices
}

#[test]
fn initialization_accepts_only_an_empty_whole_disk() {
    let empty = r#"{"blockdevices":[{"name":"sdb","path":"/dev/sdb","type":"disk","mountpoints":[null],"ro":false,"serial":"S1","size":1024}]}"#;
    assert!(blank_disk(&listed(empty), "sdb", "/S1/1024").is_ok());

    for unsafe_disk in [
        r#"{"blockdevices":[{"name":"sdb","path":"/dev/sdb","type":"disk","mountpoints":["/"],"ro":false,"serial":"S1","size":1024}]}"#,
        r#"{"blockdevices":[{"name":"sdb","path":"/dev/sdb","type":"disk","fstype":"ext4","ro":false,"serial":"S1","size":1024}]}"#,
        r#"{"blockdevices":[{"name":"sdb","path":"/dev/sdb","type":"disk","ro":false,"serial":"S1","size":1024,"children":[{"name":"sdb1","path":"/dev/sdb1","type":"part"}]}]}"#,
        r#"{"blockdevices":[{"name":"sdb","path":"/dev/sdb","type":"disk","ro":true,"serial":"S1","size":1024}]}"#,
    ] {
        assert!(blank_disk(&listed(unsafe_disk), "sdb", "/S1/1024").is_err());
    }
    assert!(blank_disk(&listed(empty), "sda", "/S1/1024").is_err());
}

#[test]
fn initialization_refuses_another_disk_that_took_the_name() {
    let other = r#"{"blockdevices":[{"name":"sdb","path":"/dev/sdb","type":"disk","mountpoints":[null],"ro":false,"serial":"S2","size":1024}]}"#;
    let reason = blank_disk(&listed(other), "sdb", "/S1/1024").expect_err("another disk");
    assert!(reason.contains("not the one that was selected"));
}

#[test]
fn initialization_request_rejects_paths_and_unsafe_labels() {
    let request = |device: &str, label: &str, identity: &str| InitializeRequest {
        device: device.to_owned(),
        label: label.to_owned(),
        identity: identity.to_owned(),
    };
    assert!(validate_initialize(&request("sdb", "Data-1", "wwn/S1/1024")).is_ok());
    assert!(validate_initialize(&request("../sda", "Data-1", "wwn/S1/1024")).is_err());
    assert!(validate_initialize(&request("sdb", "bad/label", "wwn/S1/1024")).is_err());
    assert!(validate_initialize(&request("sdb", "Data-1", "")).is_err());
    assert!(validate_initialize(&request("sdb", "Data-1", "a\nb")).is_err());
}

#[test]
fn a_volume_fstab_needs_keeps_its_label() {
    let devices = listed(
        r#"{"blockdevices":[{"name":"sdb","path":"/dev/sdb","type":"disk","children":[
            {"name":"sdb1","path":"/dev/sdb1","type":"part","fstype":"ext4","uuid":"D1","label":"Archive"},
            {"name":"sdb2","path":"/dev/sdb2","type":"part","fstype":"ext4","uuid":"D2","label":"Media"},
            {"name":"sdb3","path":"/dev/sdb3","type":"part","fstype":"ext4","uuid":"SYS","label":"Var"},
            {"name":"sdb4","path":"/dev/sdb4","type":"part","fstype":"exfat","uuid":"D4"}
        ]}]}"#,
    );
    let fstab =
        "LABEL=Media /mnt/media ext4 defaults,nofail 0 2\nUUID=SYS /var ext4 defaults 0 2\n";
    assert!(renamable(&devices, "sdb1", fstab).is_ok());
    assert!(renamable(&devices, "sdb2", fstab)
        .expect_err("mounted by label")
        .contains("fstab"));
    assert!(renamable(&devices, "sdb3", fstab).is_err());
    assert!(renamable(&devices, "sdb4", fstab).is_err());
    assert!(renamable(&devices, "sdz1", fstab).is_err());
}

#[test]
fn rename_request_accepts_only_device_names_and_ext_labels() {
    assert!(validate_rename(&RenameRequest {
        device: "sdb1".to_owned(),
        label: "Archive-1".to_owned(),
    })
    .is_ok());
    assert!(validate_rename(&RenameRequest {
        device: "../sda1".to_owned(),
        label: "Archive-1".to_owned(),
    })
    .is_err());
    assert!(validate_rename(&RenameRequest {
        device: "sdb1".to_owned(),
        label: "bad/label".to_owned(),
    })
    .is_err());
}

#[test]
fn smart_request_accepts_only_device_names() {
    assert!(valid_device_name("nvme0n1"));
    assert!(valid_device_name("sdb"));
    assert!(!valid_device_name("../sda"));
    assert!(!valid_device_name(""));
}

#[test]
fn an_initialization_command_records_the_console_and_the_exception() {
    let directory = std::env::temp_dir().join(format!("nas-storage-log-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&directory);
    let log = directory.join("operation.log");

    let failure = output_logged(
        "sh",
        &[
            "-c",
            "printf 'partial output'; printf 'device refused' >&2; exit 7",
        ],
        &log,
    )
    .expect_err("the command must fail");
    let recorded = std::fs::read_to_string(&log).expect("the diagnostic log");

    assert!(failure.contains("device refused"));
    assert!(recorded.contains("$ sh -c"));
    assert!(recorded.contains("exit code: 7"));
    assert!(recorded.contains("stdout:\npartial output"));
    assert!(recorded.contains("stderr:\ndevice refused"));
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn excludes_system_descendants_but_keeps_data_siblings() {
    let devices = r#"{"blockdevices":[{"type":"disk","children":[
        {"type":"part","fstype":"vfat","uuid":"BOOT","mountpoints":["/boot/efi"]},
        {"type":"part","fstype":"crypto_LUKS","uuid":"CRYPT","children":[
            {"type":"crypt","fstype":"LVM2_member","children":[
                {"type":"lvm","fstype":"ext4","uuid":"SYSTEM","mountpoints":["/"]}
            ]}
        ]},
        {"type":"part","fstype":"ext4","uuid":"DATA-1","label":"Archivio","mountpoints":[null]},
        {"type":"part","fstype":"swap","uuid":"SWAP","mountpoints":[null]}
    ]}]}"#;
    let result = classify(devices).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].uuid, "DATA-1");
    assert_eq!(result[0].name, "Archivio");
}

#[test]
fn ignores_missing_filesystem_read_only_and_duplicate_uuid() {
    let devices = r#"{"blockdevices":[
        {"type":"part","fstype":null,"uuid":null},
        {"type":"part","fstype":"ext4","uuid":"RO","ro":true},
        {"type":"part","fstype":"ext4","uuid":"SAME"},
        {"type":"part","fstype":"xfs","uuid":"SAME"},
        {"type":"part","fstype":"ext4","uuid":"GOOD","mountpoints":["/run/media/alice/data"]}
    ]}"#;
    let result = classify(devices).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].uuid, "GOOD");
    assert_eq!(
        result[0].mountpoint.as_deref(),
        Some(Path::new("/run/media/alice/data"))
    );
}

#[test]
fn publishes_formatted_whole_data_disks_but_not_host_state() {
    let devices = r#"{"blockdevices":[
        {"type":"disk","fstype":"ext4","uuid":"RAW","mountpoints":[null]},
        {"type":"disk","fstype":"ext4","uuid":"ROOT","mountpoints":["/"]},
        {"type":"disk","fstype":null,"uuid":null},
        {"type":"part","fstype":"ext4","uuid":"STATE","mountpoints":["/var/lib/nas-os"]},
        {"type":"part","fstype":"ext4","uuid":"OTHER","mountpoints":["/data"]}
    ]}"#;
    let result = classify(devices).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].uuid, "OTHER");
    assert_eq!(result[1].uuid, "RAW");
}

#[test]
fn protects_system_uuid_even_when_unmounted() {
    let devices = r#"{"blockdevices":[
        {"type":"part","fstype":"ext4","uuid":"SYSTEM","mountpoints":[null]},
        {"type":"disk","fstype":"ext4","uuid":"SYSTEM-DISK","mountpoints":[null]},
        {"type":"part","fstype":"ext4","uuid":"DATA","mountpoints":[null]}
    ]}"#;
    let protected = fstab_system_uuids(
        "UUID=SYSTEM / ext4 defaults 0 1\nUUID=SYSTEM-DISK /var ext4 defaults 0 2\nUUID=DATA /mnt/data ext4 defaults 0 2\n",
    );
    let result = classify_with_system_uuids(devices, &protected).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].uuid, "DATA");
}

#[test]
fn keeps_one_raid_volume_repeated_in_topology_but_rejects_cloned_filesystems() {
    let devices = r#"{"blockdevices":[
        {"type":"disk","children":[{"type":"raid1","path":"/dev/md0","fstype":"ext4","uuid":"RAID"}]},
        {"type":"disk","children":[{"type":"raid1","path":"/dev/md0","fstype":"ext4","uuid":"RAID"}]},
        {"type":"part","path":"/dev/sdb1","fstype":"ext4","uuid":"CLONE"},
        {"type":"part","path":"/dev/sdc1","fstype":"ext4","uuid":"CLONE"}
    ]}"#;
    let result = classify(devices).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].uuid, "RAID");
}

#[tokio::test]
async fn the_wizard_user_comes_from_the_terminal_drop_in_and_not_from_the_database() {
    let dir = std::env::temp_dir().join(format!("nas-storage-dropin-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("directory");
    let dropin = dir.join("user.conf");

    assert_eq!(wizard_user(&dropin).await, Ok(None));

    std::fs::write(&dropin, "[Service]\n").expect("written");
    assert_eq!(wizard_user(&dropin).await, Ok(None));

    std::fs::write(&dropin, "[Service]\nUser=../root\nGroup=root\n").expect("written");
    assert_eq!(
        wizard_user(&dropin).await,
        Err("invalid wizard user".to_owned())
    );
    std::fs::remove_dir_all(&dir).expect("removed");
}

#[test]
fn a_folder_with_nothing_mounted_is_closed_to_everyone() {
    let root = std::env::temp_dir().join(format!("nas-storage-seal-{}", std::process::id()));
    let gone = root.join("v-gone");
    std::fs::create_dir_all(&gone).expect("folder");
    std::fs::write(root.join("note"), "kept").expect("file");

    seal_unmounted(&root);

    let mode = |path: &Path| std::fs::metadata(path).expect("metadata").mode() & 0o777;
    assert_eq!(mode(&gone), 0);
    assert_ne!(mode(&root.join("note")), 0);
    std::fs::set_permissions(&gone, std::fs::Permissions::from_mode(0o700)).expect("reopened");
    std::fs::remove_dir_all(&root).expect("removed");
    seal_unmounted(&root);
}
