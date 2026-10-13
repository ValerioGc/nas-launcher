//! Access rules and refused sign-ins.

use crate::{
    apps::services::nas_system_settings::access::{self, NewRule, Rule},
    http::{
        error::ApiFailure,
        extract::{Peer, ValidJson},
        state::AppState,
    },
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

/// The addresses allowed to reach the NAS
#[utoipa::path(get, path = "/access/rules", responses((status = OK, body = Vec<Rule>)))]
async fn list(State(db): State<SqlitePool>) -> Result<Json<Vec<Rule>>, ApiFailure> {
    access::list(&db).await.map(Json)
}

/// Adds an allowed address and answers with the whole list
#[utoipa::path(
    post,
    path = "/access/rules",
    request_body = NewRule,
    responses((status = OK, body = Vec<Rule>))
)]
async fn add(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<NewRule>,
) -> Result<Json<Vec<Rule>>, ApiFailure> {
    access::add(&db, input).await.map(Json)
}

/// Removes an allowed address and answers with the whole list
#[utoipa::path(delete, path = "/access/rules/{id}", responses((status = OK, body = Vec<Rule>)))]
async fn remove(
    State(db): State<SqlitePool>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Rule>>, ApiFailure> {
    access::remove(&db, &id).await.map(Json)
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
enum Field {
    Username,
    Password,
}
impl Field {
    fn id(&self) -> &'static str {
        match self {
            Self::Username => "username",
            Self::Password => "password",
        }
    }
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct Failure {
    field: Field,
}

#[derive(Serialize, ToSchema)]
struct Recorded {
    recorded: bool,
}

/// Records a refused sign-in
#[utoipa::path(
    post,
    path = "/access/failures",
    request_body = Failure,
    responses((status = OK, body = Recorded))
)]
async fn failure(
    State(db): State<SqlitePool>,
    Peer(address): Peer,
    ValidJson(input): ValidJson<Failure>,
) -> Json<Recorded> {
    let recorded = access::record_failure(&db, address, input.field.id()).await;
    Json(Recorded { recorded })
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, add))
        .routes(routes!(remove))
        .routes(routes!(failure))
}
