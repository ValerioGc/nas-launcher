use super::*;
use crate::services::storage::inventory::Devices;
use std::os::unix::fs::symlink;

fn temporary(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("nas-crypt-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("directory");
    path
}

fn disk(json: &str) -> Device {
    serde_json::from_str::<Devices>(json)
        .expect("listed")
        .blockdevices
        .remove(0)
}

#[test]
fn base64_pads_the_last_group() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    assert_eq!(base64(&[0xff, 0xfe, 0x00]), "//4A");
}

#[test]
fn a_recovery_key_is_hexadecimal_in_groups_of_eight() {
    let key = recovery_key().expect("key");
    let groups: Vec<_> = key.split('-').collect();

    assert_eq!(groups.len(), 8);
    assert!(groups
        .iter()
        .all(|group| group.len() == 8 && group.bytes().all(|byte| byte.is_ascii_hexdigit())));
    assert_ne!(key, recovery_key().expect("another key"));
}

#[test]
fn the_slots_come_from_the_metadata_of_luks2() {
    assert_eq!(
        slots_of(r#"{"keyslots":{"2":{},"0":{},"1":{}},"tokens":{}}"#),
        Ok(vec![0, 1, 2])
    );
    assert!(slots_of("not json").is_err());
    assert!(slots_of(r#"{"tokens":{}}"#).is_err());
}

#[test]
fn the_keys_of_a_container_follow_its_slots() {
    let closed = HashSet::from(["ab".to_owned()]);

    let keys = keys_of(Some(&[0, 2]), "ab", &closed);
    assert!(keys.password && keys.recovery && keys.held);

    assert!(!keys_of(Some(&[1]), "cd", &closed).automatic);
    assert!(!keys_of(Some(&[1]), "cd", &closed).password);

    let unknown = keys_of(None, "cd", &closed);
    assert!(unknown.password && !unknown.held);
}

#[test]
fn a_secret_lives_in_the_runtime_directory_and_is_gone_when_dropped() {
    let runtime = temporary("runtime");
    std::env::set_var("RUNTIME_DIRECTORY", &runtime);
    let path = {
        let secret = Secret::write("key", b"long secret").expect("secret");
        let mode = fs::metadata(&secret.0).expect("metadata").permissions();
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(&mode) & 0o777,
            0o600
        );
        assert_eq!(fs::read(&secret.0).expect("read"), b"long secret");
        secret.0.clone()
    };
    assert!(!path.exists());
    fs::remove_dir_all(&runtime).expect("removed");
}

#[test]
fn the_processes_under_a_volume_are_named_by_their_unit() {
    let proc = temporary("proc");
    let volume = temporary("volume");
    let process = |pid: &str, cwd: &Path, cgroup: &str, comm: &str| {
        let directory = proc.join(pid);
        fs::create_dir_all(directory.join("fd")).expect("process");
        symlink(cwd, directory.join("cwd")).expect("cwd");
        fs::write(directory.join("cgroup"), cgroup).expect("cgroup");
        fs::write(directory.join("comm"), comm).expect("comm");
    };
    process(
        "101",
        &volume.join("photos"),
        "0::/system.slice/nas-app@nas-backup-manager.service\n",
        "nas-backup",
    );
    process("102", &volume, "0::/user.slice/session-2.scope\n", "bash\n");
    process(
        "103",
        Path::new("/tmp"),
        "0::/system.slice/cron.service\n",
        "cron",
    );
    symlink(volume.join("open.db"), proc.join("103/fd/4")).expect("fd");
    process("104", Path::new("/"), "0::/init.scope\n", "systemd");
    fs::create_dir_all(proc.join("self")).expect("not a process");

    let mut users = users_of(&proc, &[volume.display().to_string()]);
    users.sort();
    assert_eq!(
        users,
        ["bash", "cron.service", "nas-app@nas-backup-manager.service"]
    );
    assert!(users_of(&proc, &[]).is_empty());
    fs::remove_dir_all(&proc).expect("removed");
    fs::remove_dir_all(&volume).expect("removed");
}

const DATA_DISK: &str = r#"{"blockdevices":[{"name":"sdb","path":"/dev/sdb","type":"disk","size":2000,"serial":"D1","tran":"sata","children":[
    {"name":"sdb1","path":"/dev/sdb1","type":"part","fstype":"ext4","uuid":"DATA"}
]}]}"#;

#[test]
fn only_an_internal_data_disk_with_its_identity_is_formatted() {
    let none = HashSet::new();
    let data = disk(DATA_DISK);
    assert!(formattable(&data, "sdb", "/D1/2000", &none).is_ok());
    assert!(formattable(&data, "sdb", "/D2/2000", &none)
        .expect_err("another disk")
        .contains("not the one"));
    assert!(formattable(
        &data,
        "sdb",
        "/D1/2000",
        &HashSet::from(["data".to_owned()])
    )
    .expect_err("system")
    .contains("system"));

    let usb = disk(
        r#"{"blockdevices":[{"name":"sdc","path":"/dev/sdc","type":"disk","size":64,"tran":"usb"}]}"#,
    );
    assert!(formattable(&usb, "sdc", "//64", &none).is_err());
    let grouped = disk(
        r#"{"blockdevices":[{"name":"sdd","path":"/dev/sdd","type":"disk","size":5,"children":[
            {"name":"sdd1","path":"/dev/sdd1","type":"part","fstype":"LVM2_member"}
        ]}]}"#,
    );
    assert!(formattable(&grouped, "sdd", "//5", &none)
        .expect_err("volume group")
        .contains("group"));
    let partition =
        disk(r#"{"blockdevices":[{"name":"sde1","path":"/dev/sde1","type":"part","size":5}]}"#);
    assert!(formattable(&partition, "sde1", "//5", &none).is_err());
}

#[test]
fn a_container_of_crypttab_or_with_the_system_belongs_to_the_system() {
    let crypttab = HashSet::from(["swap-luks".to_owned()]);
    let none = HashSet::new();
    let root = disk(
        r#"{"blockdevices":[{"name":"sda2","type":"part","fstype":"crypto_LUKS","uuid":"ROOT-LUKS","children":[
            {"name":"root","type":"crypt","fstype":"ext4","mountpoints":["/"]}
        ]}]}"#,
    );
    let swap = disk(
        r#"{"blockdevices":[{"name":"sda3","type":"part","fstype":"crypto_LUKS","uuid":"SWAP-LUKS"}]}"#,
    );
    let data = disk(
        r#"{"blockdevices":[{"name":"sdb1","type":"part","fstype":"crypto_LUKS","uuid":"DATA-LUKS"}]}"#,
    );

    assert!(system_container(&root, &none, &crypttab));
    assert!(system_container(&swap, &none, &crypttab));
    assert!(!system_container(&data, &none, &crypttab));
}
