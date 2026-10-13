use super::*;

fn asked(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn accepts_only_the_installable_packages_once_each() {
    assert_eq!(checked(&asked(&["msmtp", "msmtp"])), Ok(vec!["msmtp"]));
    assert_eq!(checked(&[]), Ok(Vec::new()));
    assert_eq!(
        checked(&asked(&["clamav-freshclam", "clamav", "clamav"])),
        Ok(vec!["clamav", "clamav-freshclam"])
    );
    for wanted in ["postfix", "msmtp-mta", "../msmtp", ""] {
        assert!(checked(&asked(&["msmtp", wanted])).is_err(), "{wanted}");
    }
}

#[tokio::test]
async fn nothing_is_installed_where_the_applications_do_not_run() {
    assert_eq!(install("nas-mail-client", &["msmtp"]).await, Ok(()));
    assert_eq!(remove("nas-mail-client").await, Ok(()));
}
