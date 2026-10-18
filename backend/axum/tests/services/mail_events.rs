use super::*;
use crate::services::manifest::Capability;

fn offering(id: &str, endpoint: &str, version: u32) -> Manifest {
    Manifest {
        id: id.to_owned(),
        name: id.to_owned(),
        system: false,
        version: "1.0.0".to_owned(),
        icon: String::new(),
        category: None,
        protocol: 1,
        provides: vec![Capability {
            capability: CAPABILITY.to_owned(),
            version,
            endpoint: endpoint.to_owned(),
        }],
        paths: Vec::new(),
        network: Vec::new(),
        packages: Vec::new(),
        background: false,
        shares: false,
        cleanup: false,
        socket: std::path::PathBuf::new(),
        exec: String::new(),
    }
}

fn package(id: &str, level: Level) -> Package {
    Package {
        id: id.to_owned(),
        current: "1".to_owned(),
        version: "2".to_owned(),
        level,
    }
}

#[test]
fn only_an_internal_endpoint_receives_the_events() {
    let manifests = [
        offering("public", "/api/events", 1),
        offering("future", "/internal/events", 2),
        offering("escaping", "/internal/../api", 1),
        offering("nas-mail-client", "/internal/events", 1),
    ];
    let (app, endpoint) = receiver(&manifests).expect("a receiver");
    assert_eq!(app.id, "nas-mail-client");
    assert_eq!(endpoint, "/internal/events");
    assert!(receiver(&manifests[..3]).is_none());
}

#[test]
fn an_event_carries_the_notice_and_when_it_happened() {
    let notice =
        Notice::new("signInRefused", "Sign-in refused", "From 10.0.0.2.").with("from", "10.0.0.2");
    let event: serde_json::Value = serde_json::from_slice(&body(&notice, 42)).expect("json");
    assert_eq!(
        event,
        serde_json::json!({
            "key": "signInRefused",
            "title": "Sign-in refused",
            "message": "From 10.0.0.2.",
            "params": { "from": "10.0.0.2" },
            "at": 42
        })
    );
}

#[test]
fn the_upgrades_are_counted_by_level_and_listed_for_the_mail() {
    let packages = [
        package("libc6", Level::Security),
        package("bash", Level::Stable),
        package("tzdata", Level::Stable),
        package("cockpit", Level::Other),
    ];
    let notice = upgrades_notice(&packages);
    assert_eq!(notice.key, "systemUpdatesAvailable");
    assert_eq!(notice.params["count"], "4");
    assert_eq!(notice.params["security"], "1");
    assert_eq!(notice.params["stable"], "2");
    assert_eq!(notice.params["other"], "1");
    assert!(!notice.params.contains_key("packages"));
    assert_eq!(
        upgrades_list(&packages[..2]),
        "libc6 1 -> 2 (security)\nbash 1 -> 2 (stable)"
    );
}
