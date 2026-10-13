use super::*;

#[test]
fn splits_the_fields_of_nmcli_keeping_escaped_colons() {
    assert_eq!(
        fields(r"home\:5G:wifi:connected"),
        ["home:5G", "wifi", "connected"]
    );
    assert_eq!(fields(r"back\\slash:x"), [r"back\slash", "x"]);
    assert_eq!(fields(""), [""]);
}

#[test]
fn writes_a_prefix_as_a_netmask() {
    assert_eq!(netmask(24), "255.255.255.0");
    assert_eq!(netmask(16), "255.255.0.0");
    assert_eq!(netmask(32), "255.255.255.255");
    assert_eq!(netmask(0), "0.0.0.0");
    assert_eq!(netmask(40), "255.255.255.255");
}

#[test]
fn reads_the_value_of_a_property_and_skips_an_empty_one() {
    assert_eq!(property("GENERAL.CONNECTION:Home"), Some("Home".to_owned()));
    assert_eq!(property("GENERAL.CONNECTION:--"), None);
    assert_eq!(
        property("GENERAL.CONNECTION:\nX:Office"),
        Some("Office".to_owned())
    );
    assert_eq!(property(""), None);
}

#[test]
fn finds_the_signal_and_the_security_of_a_network() {
    let list = "Office:40:WPA2\nHome:72:WPA2 WPA3\nOpen:30:\nbroken";
    assert_eq!(
        parse_wifi_details(list, "Home"),
        (Some(72), Some("WPA2 WPA3".to_owned()))
    );
    assert_eq!(parse_wifi_details(list, "Open"), (Some(30), None));
    assert_eq!(parse_wifi_details(list, "Away"), (None, None));
}

#[tokio::test]
async fn reads_this_machine_without_changing_it() {
    if let Some(interface) = read().await {
        assert!(!interface.name.is_empty());
        assert!(interface.ipv4.contains('.'));
    }
    match scan(false).await {
        Ok(found) => assert!(found.iter().all(|network| !network.ssid.is_empty())),
        Err(problem) => assert!(matches!(
            problem,
            WifiProblem::NoNetworkManager | WifiProblem::NoDevice | WifiProblem::ListFailed
        )),
    }
    let _ = wired_available().await;
    if let Some((interface, _)) = wifi_connection().await {
        assert!(!interface.is_empty());
    }
    assert!(run("false", &[]).await.is_none());
    assert!(run_checked("false", &[]).await.is_err());
    assert!(run_checked("nas-os-missing-program", &[]).await.is_err());
}
