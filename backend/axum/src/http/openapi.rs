//! The OpenAPI document of the launcher generated from the annotated handlers.

use crate::http::error::ApiError;
use utoipa::{
    openapi::{path::Operation, Info, OpenApi, RefOr, Response, ResponseBuilder},
    PartialSchema, ToSchema,
};

fn error_response(description: &str) -> RefOr<Response> {
    ResponseBuilder::new()
        .description(description)
        .content(
            "application/json",
            utoipa::openapi::ContentBuilder::new()
                .schema(Some(utoipa::openapi::Ref::from_schema_name(
                    ApiError::name(),
                )))
                .build(),
        )
        .build()
        .into()
}

fn operation_id(method: &str, path: &str) -> String {
    let mut id = method.to_owned();
    let path = path.strip_prefix("/api").unwrap_or(path);
    for word in path.split(|c: char| !c.is_ascii_alphanumeric()) {
        let mut letters = word.chars();
        if let Some(first) = letters.next() {
            id.push(first.to_ascii_uppercase());
            id.extend(letters);
        }
    }
    id
}

fn complete(method: &str, path: &str, operation: &mut Operation) {
    operation.operation_id = Some(operation_id(method, path));
    let responses = &mut operation.responses.responses;
    for response in responses.values_mut() {
        if let RefOr::T(response) = response {
            if response.description.is_empty() {
                response.description = "Success".to_owned();
            }
        }
    }
    responses
        .entry("4XX".to_owned())
        .or_insert_with(|| error_response("The request was refused"));
    responses
        .entry("5XX".to_owned())
        .or_insert_with(|| error_response("The launcher or the machine could not answer"));
}

/// Names the document and gives every operation a unique id and the shared error body.
pub fn document(mut api: OpenApi) -> OpenApi {
    api.info = Info::new("nas-os launcher", env!("CARGO_PKG_VERSION"));
    for (path, item) in api.paths.paths.iter_mut() {
        for (method, operation) in [
            ("get", &mut item.get),
            ("put", &mut item.put),
            ("post", &mut item.post),
            ("patch", &mut item.patch),
            ("delete", &mut item.delete),
        ] {
            if let Some(operation) = operation {
                complete(method, path, operation);
            }
        }
    }
    let components = api.components.get_or_insert_with(Default::default);
    components
        .schemas
        .insert(ApiError::name().into_owned(), ApiError::schema());
    api
}

#[cfg(test)]
#[path = "../../../tests/http/openapi.rs"]
mod tests;
