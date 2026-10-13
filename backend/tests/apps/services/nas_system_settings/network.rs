use super::*;
use crate::services::database;
use axum::http::StatusCode;

#[tokio::test]
async fn reads_the_network_and_offers_no_change_unless_it_is_real() {
    match status(false).await {
        Ok(state) => {
            assert!(state.connection.connected);
            assert!(!state.connection.interface.is_empty());
            assert!(!state.can_connect);
            assert!(!state.can_disconnect);
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
    for refused in [
        connect_wifi(&db, request).await.err(),
        connect_wired(&db).await.err(),
        disconnect_wifi(&db).await.err(),
    ] {
        assert_eq!(refused.expect("refused").0, StatusCode::SERVICE_UNAVAILABLE);
    }
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
