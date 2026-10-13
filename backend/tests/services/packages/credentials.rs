use super::*;

#[tokio::test]
async fn the_launcher_token_comes_from_its_credential_and_otherwise_from_a_development_file() {
    let directory = std::env::temp_dir().join(format!("nas-os-credentials-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("temporary directory");
    let development = directory.join("development-token");
    std::fs::write(&development, " ghp_development \n").expect("development token");

    unsafe {
        std::env::remove_var("CREDENTIALS_DIRECTORY");
        std::env::set_var("NAS_LAUNCHER_TOKEN_PATH", &development);
    }
    assert_eq!(launcher().await.as_deref(), Some("ghp_development"));

    std::fs::write(directory.join(LAUNCHER_CREDENTIAL), "ghp_credential").expect("credential");
    unsafe {
        std::env::set_var("CREDENTIALS_DIRECTORY", &directory);
    }
    assert_eq!(launcher().await.as_deref(), Some("ghp_credential"));

    std::fs::write(directory.join(LAUNCHER_CREDENTIAL), "  \n").expect("credential");
    std::fs::remove_file(&development).expect("development token");
    assert_eq!(launcher().await, None);

    unsafe {
        std::env::remove_var("CREDENTIALS_DIRECTORY");
        std::env::remove_var("NAS_LAUNCHER_TOKEN_PATH");
    }
    std::fs::remove_dir_all(&directory).expect("cleanup");
}

#[test]
fn a_token_is_one_word_of_printable_characters() {
    assert!(validate("ghp_abc123").is_ok());
    assert!(validate("").is_err());
    assert!(validate("two words").is_err());
    assert!(validate("line\nbreak").is_err());
    assert!(validate(&"x".repeat(513)).is_err());
}

#[tokio::test]
async fn the_token_left_for_the_root_unit_is_readable_by_the_launcher_only() {
    use std::os::unix::fs::PermissionsExt;

    let path = std::env::temp_dir().join(format!("nas-os-token-{}", std::process::id()));
    keep(&path, "ghp_new").await.expect("written");

    assert_eq!(std::fs::read_to_string(&path).expect("token"), "ghp_new");
    let mode = std::fs::metadata(&path)
        .expect("metadata")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    std::fs::remove_file(&path).expect("cleanup");
}
