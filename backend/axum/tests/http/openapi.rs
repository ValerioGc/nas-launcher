use super::*;
use crate::http::routes::openapi;

#[test]
fn an_operation_is_named_after_its_method_and_path() {
    assert_eq!(operation_id("get", "/health"), "getHealth");
    assert_eq!(
        operation_id("post", "/api/apps/{id}/{action}"),
        "postAppsIdAction"
    );
}

#[test]
fn every_operation_has_its_own_id_and_the_shared_errors() {
    let api = openapi();
    let mut ids = Vec::new();
    for item in api.paths.paths.values() {
        for operation in [&item.get, &item.put, &item.post, &item.patch, &item.delete]
            .into_iter()
            .flatten()
        {
            ids.push(operation.operation_id.clone().expect("an id"));
            assert!(operation.responses.responses.contains_key("4XX"));
            assert!(operation.responses.responses.contains_key("5XX"));
        }
    }
    let count = ids.len();
    ids.sort();
    ids.dedup();

    assert_eq!(ids.len(), count);
}

#[test]
fn the_committed_document_matches_the_code() {
    let committed = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/development/openapi.json"
    ))
    .expect("the committed document");
    let committed: serde_json::Value = serde_json::from_str(&committed).expect("valid JSON");

    assert!(
        committed == serde_json::to_value(openapi()).expect("a document"),
        "docs/development/openapi.json is out of date: run pnpm run api:generate"
    );
}
