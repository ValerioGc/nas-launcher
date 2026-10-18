use super::*;
use crate::services::database;
use axum::http::StatusCode;

#[tokio::test]
async fn reads_the_network_and_offers_no_change_unless_it_is_real() {
    let db = database::open_in_memory().await.expect("database");
    match status(&db, false).await {
        Ok(state) => {
            assert!(state.connection.connected);
            assert!(!state.connection.interface.is_empty());
            assert!(!state.can_connect);
            assert!(!state.can_disconnect);
            assert!(state.adapters.iter().all(|adapter| !adapter.can_toggle));
            assert!(state.wifi_problem.is_none() || state.wifi.is_empty());
        }
        Err(failure) => assert_eq!(failure.0, StatusCode::SERVICE_UNAVAILABLE),
    }
}

#[tokio::test]
async fn refuses_every_change_of_the_network_unless_it_is_real() {
    let db = database::open_in_memory().await.expect("database");
    let request = WifiRequest {
        ssid: "home".to_owned(),
        password: Some("long enough".to_owned()),
    };
    let adapter = AdapterRequest {
        interface: "wlan0".to_owned(),
        enabled: false,
    };
    for refused in [
        connect_wifi(&db, request).await.err(),
        connect_wired(&db).await.err(),
        disconnect_wifi(&db).await.err(),
        set_adapter(&db, adapter).await.err(),
    ] {
        assert_eq!(refused.expect("refused").0, StatusCode::SERVICE_UNAVAILABLE);
    }
    assert!(disabled(&db).await.is_empty());
}

fn device(name: &str, wireless: bool, state: &str) -> machine::Device {
    machine::Device {
        name: name.to_owned(),
        wireless,
        state: state.to_owned(),
    }
}

#[test]
fn reads_the_state_of_a_card_as_nmcli_writes_it() {
    assert_eq!(
        AdapterState::from_nmcli("connected"),
        AdapterState::Connected
    );
    assert_eq!(
        AdapterState::from_nmcli("unavailable"),
        AdapterState::Unavailable
    );
    assert_eq!(
        AdapterState::from_nmcli("unmanaged"),
        AdapterState::Unmanaged
    );
    for other in ["disconnected", "connecting (configuring)", "deactivating"] {
        assert_eq!(AdapterState::from_nmcli(other), AdapterState::Disconnected);
    }
}

#[test]
fn never_offers_to_turn_off_the_card_in_use_nor_one_left_to_the_system() {
    let devices = [
        device("eth0", false, "connected"),
        device("wlan0", true, "disconnected"),
        device("enp1s0", false, "unmanaged"),
    ];
    let listed = adapters(&devices, "eth0", &["wlan0".to_owned()]);
    assert_eq!(listed[0].kind, LinkKind::Lan);
    assert!(listed[0].in_use && listed[0].enabled);
    assert_eq!(listed[1].kind, LinkKind::Wifi);
    assert!(!listed[1].in_use && !listed[1].enabled);
    assert_eq!(listed[2].state, AdapterState::Unmanaged);

    assert!(listed.iter().all(|adapter| !adapter.can_toggle));
}

#[test]
fn keeps_a_card_off_only_while_another_one_carries_the_network() {
    let wifi = device("wlan0", true, "connected");
    assert_eq!(keep_off(&wifi, Some("eth0")), Some(false));
    assert_eq!(keep_off(&wifi, Some("wlan0")), None);
    assert_eq!(keep_off(&wifi, None), None);
    let resting = device("wlan0", true, "disconnected");
    assert_eq!(keep_off(&resting, Some("eth0")), None);

    assert_eq!(keep_off(&resting, None), Some(true));
}

#[tokio::test]
async fn reads_the_cards_turned_off_from_their_settings() {
    let db = database::open_in_memory().await.expect("database");
    settings::write(
        &db,
        Section::Network,
        &NetworkSettings {
            disabled: vec!["wlan0".to_owned()],
        },
    )
    .await
    .expect("saved");
    assert_eq!(disabled(&db).await, ["wlan0"]);
    assert!(disabled_now("wlan0", &disabled(&db).await));
    assert!(!disabled_now("eth0", &disabled(&db).await));
}

#[tokio::test]
async fn tells_a_refused_password_from_a_network_that_failed() {
    let db = database::open_in_memory().await.expect("database");

    let password = refused(&db, "Secrets were required, but not provided".to_owned()).await;
    assert_eq!(password.0, StatusCode::BAD_REQUEST);
    let other = refused(&db, "no carrier".to_owned()).await;
    assert_eq!(other.0, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(other.1, "no carrier");

    let recorded: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE key = 'networkFailed'")
            .fetch_one(&db)
            .await
            .expect("notifications");
    assert_eq!(recorded, 2);
}
