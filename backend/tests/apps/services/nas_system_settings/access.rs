use super::*;

#[test]
fn a_value_is_stored_in_one_spelling_only() {
    assert_eq!(
        normalise(RuleKind::Ip, " 192.168.1.5 ").unwrap(),
        "192.168.1.5"
    );
    assert_eq!(
        normalise(RuleKind::Range, "10.0.0.0 / 8").unwrap(),
        "10.0.0.0/8"
    );
}

#[test]
fn what_is_not_an_address_is_refused() {
    assert!(normalise(RuleKind::Ip, "192.168.1").is_err());
    assert!(normalise(RuleKind::Range, "192.168.1.0/33").is_err());
    assert!(normalise(RuleKind::Range, "192.168.1.0").is_err());
}

#[test]
fn a_range_covers_the_addresses_inside_it_and_no_others() {
    let (network, prefix) = split_range("192.168.1.0/24").unwrap();
    assert!(within("192.168.1.1".parse().unwrap(), network, prefix));
    assert!(within("192.168.1.255".parse().unwrap(), network, prefix));
    assert!(!within("192.168.2.1".parse().unwrap(), network, prefix));
    assert!(!within("::1".parse().unwrap(), network, prefix));
}

#[test]
fn a_prefix_that_does_not_end_on_a_byte_still_cuts_in_the_right_place() {
    let (network, prefix) = split_range("10.0.0.0/12").unwrap();
    assert!(within("10.15.255.1".parse().unwrap(), network, prefix));
    assert!(!within("10.16.0.1".parse().unwrap(), network, prefix));
}

#[test]
fn an_ipv6_range_works_the_same_way() {
    let (network, prefix) = split_range("2001:db8::/32").unwrap();
    assert!(within("2001:db8:1::5".parse().unwrap(), network, prefix));
    assert!(!within("2001:db9::5".parse().unwrap(), network, prefix));
}

#[test]
fn a_hardware_address_is_not_a_kind_of_rule() {
    let input = r#"{"kind":"mac","value":"aa:bb:cc:dd:ee:ff"}"#;

    assert!(serde_json::from_str::<NewRule>(input).is_err());
}

#[tokio::test]
async fn the_table_refuses_a_hardware_address_too() {
    let db = database::open_in_memory()
        .await
        .expect("in-memory database");
    let inserted = sqlx::query(
        "INSERT INTO access_rules (kind, value, created_at) VALUES ('mac', 'aa:bb:cc:dd:ee:ff', 0)",
    )
    .execute(&db)
    .await;

    assert!(inserted.is_err());
}
