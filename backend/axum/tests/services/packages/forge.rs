use super::*;

#[test]
fn a_tag_and_a_version_are_the_same_thing() {
    assert_eq!(version("v1.2.3"), "1.2.3");
    assert_eq!(version(" 1.2.3 "), "1.2.3");
}

#[test]
fn versions_are_compared_by_number_and_not_by_text() {
    assert!(is_newer("1.10.0", "1.9.0"));
    assert!(!is_newer("1.9.0", "1.10.0"));
    assert!(!is_newer("1.2.3", "1.2.3"));
    assert!(is_newer("2.0.0", "1.99.99"));
    assert!(is_newer("1.2.1", "1.2"));
}

#[test]
fn an_unknown_version_is_older_than_every_release() {
    assert!(is_newer("0.0.1", ""));
    assert!(!is_newer("", "0.0.1"));
}

#[test]
fn the_token_travels_only_to_the_forges() {
    let token = Some("secret");
    assert_eq!(
        for_forge("https://api.github.com/repos/a/b/releases/assets/1", token),
        token
    );
    assert_eq!(
        for_forge("https://gitlab.com/a/b/-/releases/v1/downloads/x", token),
        token
    );
    assert_eq!(
        for_forge("https://downloads.example.com/x.tar.gz", token),
        None
    );
    assert_eq!(for_forge("http://api.github.com/x", token), None);
    assert_eq!(for_forge("not a url", token), None);
}

fn published(names: &[&str]) -> Release {
    Release {
        version: "0.1.3".to_owned(),
        assets: names
            .iter()
            .map(|name| Asset {
                name: (*name).to_owned(),
                url: format!("https://example.com/{name}"),
            })
            .collect(),
    }
}

#[test]
fn an_asset_named_with_other_case_or_separators_is_still_found_when_it_is_the_only_one() {
    let release = published(&[
        "disk-manager-0.1.3-linux-x64.tar.gz",
        "disk-manager-0.1.3-linux-x64.tar.gz.sha256",
        "SHA256SUMS.txt",
    ]);
    let found = release
        .asset_like("diskManager-0.1.3-linux-x64.tar.gz")
        .expect("the asset");
    assert_eq!(found.name, "disk-manager-0.1.3-linux-x64.tar.gz");
    assert!(release.asset(&format!("{}.sha256", found.name)).is_some());

    assert!(release
        .asset_like("photos-0.1.3-linux-x64.tar.gz")
        .is_none());
    let twice = published(&["disk-manager-1.tar.gz", "disk_manager-1.tar.gz"]);
    assert!(twice.asset_like("diskManager-1.tar.gz").is_none());
    assert_eq!(
        twice
            .asset_like("disk_manager-1.tar.gz")
            .map(|asset| asset.name.as_str()),
        Some("disk_manager-1.tar.gz")
    );
}
