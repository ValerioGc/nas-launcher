pub mod access;
pub mod config;
pub mod network;
pub mod power;
pub mod reset;

use crate::{
    apps::services::nas_system_settings::{
        self,
        system_updates::{self, Job, SystemUpdates},
        Access, Discovery, General, NasUpdates, Power,
    },
    http::error::ApiFailure,
    http::extract::ValidJson,
    http::state::AppState,
    services::session::Session,
};
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};

macro_rules! section {
    ($path:literal, $get:ident, $put:ident, $load:ident, $save:ident, $ty:ty) => {
        #[utoipa::path(get, path = $path, responses((status = OK, body = $ty)))]
        async fn $get(State(db): State<SqlitePool>) -> Result<Json<$ty>, ApiFailure> {
            Ok(Json(nas_system_settings::$load(&db).await?))
        }
        #[utoipa::path(put, path = $path, request_body = $ty, responses((status = OK, body = $ty)))]
        async fn $put(
            State(db): State<SqlitePool>,
            ValidJson(input): ValidJson<$ty>,
        ) -> Result<Json<$ty>, ApiFailure> {
            Ok(Json(nas_system_settings::$save(&db, input).await?))
        }
    };
}
section!(
    "/settings/general",
    general,
    save_general,
    load_general,
    save_general,
    General
);
section!(
    "/settings/access",
    access,
    save_access,
    load_access,
    save_access,
    Access
);
section!(
    "/settings/discovery",
    discovery,
    save_discovery,
    load_discovery,
    save_discovery,
    Discovery
);
section!(
    "/settings/power",
    power,
    save_power,
    load_power,
    save_power,
    Power
);

/// The Debian upgrades available and the job in progress
#[utoipa::path(get, path = "/system/updates", responses((status = OK, body = SystemUpdates)))]
async fn updates(State(s): State<Session>) -> Result<Json<SystemUpdates>, ApiFailure> {
    system_updates::state(&s).await.map(Json)
}

/// Refreshes the package index
#[utoipa::path(
    post,
    path = "/system/updates/refresh",
    responses((status = OK, body = SystemUpdates))
)]
async fn refresh_updates(
    State(s): State<Session>,
    State(db): State<SqlitePool>,
) -> Result<Json<SystemUpdates>, ApiFailure> {
    system_updates::start(&db, &s, Job::Refresh, &[])
        .await
        .map(Json)
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct UpdateSelection {
    ids: Vec<String>,
}

/// Installs the chosen upgrades
#[utoipa::path(
    post,
    path = "/system/updates",
    request_body = UpdateSelection,
    responses((status = OK, body = SystemUpdates))
)]
async fn apply_updates(
    State(s): State<Session>,
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<UpdateSelection>,
) -> Result<Json<SystemUpdates>, ApiFailure> {
    system_updates::start(&db, &s, Job::Upgrade, &input.ids)
        .await
        .map(Json)
}

/// The components of the NAS (launcher, gateway, wizard) and the updates the last check found
#[utoipa::path(get, path = "/launcher/updates", responses((status = OK, body = NasUpdates)))]
async fn launcher_update(State(s): State<Session>) -> Json<NasUpdates> {
    let session = s.0.read().await;
    Json(nas_system_settings::updates::state(&session).await)
}

/// Checks NAS component repositories for available releases.
#[utoipa::path(
    post,
    path = "/launcher/updates/check",
    responses((status = OK, body = NasUpdates))
)]
async fn check_launcher_update(
    State(s): State<Session>,
    State(db): State<SqlitePool>,
) -> Result<Json<NasUpdates>, ApiFailure> {
    let mut session = s.0.write().await;
    nas_system_settings::updates::check(&db, &mut session)
        .await
        .map(Json)
}
#[derive(Serialize, ToSchema)]
struct UpdateStarted {
    accepted: bool,
    task: String,
}

/// Installs available wizard, gateway, and launcher updates in dependency order.
#[utoipa::path(post, path = "/launcher/updates", responses((status = OK, body = UpdateStarted)))]
async fn apply_launcher_update(
    State(s): State<Session>,
    State(db): State<SqlitePool>,
) -> Result<Json<UpdateStarted>, ApiFailure> {
    let task = nas_system_settings::updates::install_updates(&db, &mut *s.0.write().await).await?;
    Ok(Json(UpdateStarted {
        accepted: true,
        task,
    }))
}

pub fn routes() -> OpenApiRouter<AppState> {
    network::routes()
        .merge(access::routes())
        .merge(power::routes())
        .merge(config::routes())
        .merge(reset::routes())
        .routes(routes!(general, save_general))
        .routes(routes!(access, save_access))
        .routes(routes!(discovery, save_discovery))
        .routes(routes!(power, save_power))
        .routes(routes!(updates, apply_updates))
        .routes(routes!(refresh_updates))
        .routes(routes!(launcher_update, apply_launcher_update))
        .routes(routes!(check_launcher_update))
}
