use crate::{
    apps::services::nas_app_manager::{self, ActionAck, App, AppDetail, ManagerSettings},
    http::error::ApiFailure,
    http::extract::ValidJson,
    http::state::AppState,
    services::packages::{self, catalogue, catalogue::CatalogueApp, credentials, logos, Offer},
    utils::{self, Mode},
};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::header,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::{
    router::{OpenApiRouter, UtoipaMethodRouterExt},
    routes,
};

/// System and installed applications
#[utoipa::path(get, path = "/apps", responses((status = OK, body = Vec<App>)))]
pub(crate) async fn installed(State(db): State<SqlitePool>) -> Result<Json<Vec<App>>, ApiFailure> {
    nas_app_manager::installed(&db).await.map(Json)
}

/// The same list the App Manager reads next to the available ones
#[utoipa::path(get, path = "/apps/installed", responses((status = OK, body = Vec<App>)))]
async fn installed_apps(db: State<SqlitePool>) -> Result<Json<Vec<App>>, ApiFailure> {
    installed(db).await
}

/// What the catalogue offers and is not installed
#[utoipa::path(get, path = "/apps/available", responses((status = OK, body = Vec<Offer>)))]
async fn available(State(db): State<SqlitePool>) -> Result<Json<Vec<Offer>>, ApiFailure> {
    nas_app_manager::available(&db).await.map(Json)
}

/// The metadata of an application
#[utoipa::path(get, path = "/apps/{id}/info", responses((status = OK, body = AppDetail)))]
async fn info(
    State(db): State<SqlitePool>,
    Path(id): Path<String>,
) -> Result<Json<AppDetail>, ApiFailure> {
    nas_app_manager::detail(&db, &id).await.map(Json)
}

fn image(mime: &str, bytes: Vec<u8>) -> Response {
    (
        [
            (header::CONTENT_TYPE, mime),
            (header::CACHE_CONTROL, "no-cache"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; style-src 'unsafe-inline'; sandbox",
            ),
        ],
        bytes,
    )
        .into_response()
}

/// The icon out of the package of an application
#[utoipa::path(
    get,
    path = "/apps/{id}/icon",
    responses((status = OK, content_type = "image/*", body = Vec<u8>))
)]
async fn icon(Path(id): Path<String>) -> Result<Response, ApiFailure> {
    let (mime, bytes) = nas_app_manager::icon(&id).await?;
    Ok(image(mime, bytes))
}

/// The logo of a catalogue entry
#[utoipa::path(
    get,
    path = "/apps/{id}/logo",
    responses((status = OK, content_type = "image/*", body = Vec<u8>))
)]
async fn logo(
    State(db): State<SqlitePool>,
    Path(id): Path<String>,
) -> Result<Response, ApiFailure> {
    let (mime, bytes) = logos::read(&db, &id).await?;
    Ok(image(&mime, bytes))
}

/// Stores the image in the body as the logo of a catalogue entry
#[utoipa::path(
    put,
    path = "/apps/{id}/logo",
    request_body(content_type = "image/*", content = Vec<u8>),
    responses((status = NO_CONTENT))
)]
async fn store_logo(
    State(db): State<SqlitePool>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<StatusCode, ApiFailure> {
    logos::store(&db, &id, &body).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Removes the logo of a catalogue entry
#[utoipa::path(delete, path = "/apps/{id}/logo", responses((status = NO_CONTENT)))]
async fn forget_logo(
    State(db): State<SqlitePool>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiFailure> {
    logos::forget(&db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Asks every repository for its latest release
#[utoipa::path(post, path = "/apps/updates", responses((status = OK, body = Vec<Offer>)))]
async fn check(State(db): State<SqlitePool>) -> Result<Json<Vec<Offer>>, ApiFailure> {
    let found = packages::check(&db).await?;
    packages::announce(&db, &found).await;
    Ok(Json(found))
}

/// The automatic update check
#[utoipa::path(get, path = "/settings/apps", responses((status = OK, body = ManagerSettings)))]
async fn settings(State(db): State<SqlitePool>) -> Result<Json<ManagerSettings>, ApiFailure> {
    nas_app_manager::load_settings(&db).await.map(Json)
}

/// Saves the automatic update check
#[utoipa::path(
    put,
    path = "/settings/apps",
    request_body = ManagerSettings,
    responses((status = OK, body = ManagerSettings))
)]
async fn save_settings(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<ManagerSettings>,
) -> Result<Json<ManagerSettings>, ApiFailure> {
    nas_app_manager::save_settings(&db, input).await.map(Json)
}

#[derive(Serialize, ToSchema)]
struct ListedApp {
    #[serde(flatten)]
    app: CatalogueApp,
    logo: String,
}

async fn listed(db: &SqlitePool, apps: Vec<CatalogueApp>) -> Json<Vec<ListedApp>> {
    let mut logos = logos::addresses(db).await;
    Json(
        apps.into_iter()
            .map(|app| ListedApp {
                logo: logos.remove(&app.id).unwrap_or_default(),
                app,
            })
            .collect(),
    )
}

/// The catalogue in effect
#[utoipa::path(get, path = "/apps/catalogue", responses((status = OK, body = Vec<ListedApp>)))]
async fn catalogue_apps(State(db): State<SqlitePool>) -> Json<Vec<ListedApp>> {
    let apps = catalogue::read(&db).await;
    listed(&db, apps).await
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct EditedCatalogue {
    apps: Vec<CatalogueApp>,
}

/// Saves the whole catalogue
#[utoipa::path(
    put,
    path = "/apps/catalogue",
    request_body = EditedCatalogue,
    responses((status = OK, body = Vec<ListedApp>))
)]
async fn save_catalogue(
    State(db): State<SqlitePool>,
    ValidJson(input): ValidJson<EditedCatalogue>,
) -> Result<Json<Vec<ListedApp>>, ApiFailure> {
    let apps = catalogue::save(&db, input.apps).await?.apps;
    let ids = apps.iter().map(|app| app.id.as_str()).collect::<Vec<_>>();
    logos::keep_only(&db, &ids).await?;
    Ok(listed(&db, apps).await)
}

#[derive(Deserialize, IntoParams)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
struct ActionOptions {
    #[serde(default)]
    remove_data: bool,
}

#[derive(Serialize, ToSchema)]
struct Closed {
    /// The applications stopped.
    stopped: Vec<String>,
}

/// Closes the windows of a session, at sign-in and sign-out: stops every application, except
/// those working in background
#[utoipa::path(post, path = "/apps/close", responses((status = OK, body = Closed)))]
async fn close_all(State(db): State<SqlitePool>) -> Result<Json<Closed>, ApiFailure> {
    nas_app_manager::close_all(&db)
        .await
        .map(|stopped| Json(Closed { stopped }))
}

/// Installs updates or removes an application or starts and stops its process
#[utoipa::path(
    post,
    path = "/apps/{id}/{action}",
    params(
        ("id" = String, Path),
        ("action" = String, Path, description = "install update uninstall start stop restart or close (stop unless it works in background)"),
        ActionOptions
    ),
    responses((status = OK, body = ActionAck))
)]
async fn action(
    State(db): State<SqlitePool>,
    Path((id, action)): Path<(String, String)>,
    Query(options): Query<ActionOptions>,
) -> Result<Json<ActionAck>, ApiFailure> {
    match action.as_str() {
        "install" | "update" => nas_app_manager::install(&db, id).await.map(Json),
        "uninstall" => nas_app_manager::uninstall(&db, id, options.remove_data)
            .await
            .map(Json),
        "start" | "stop" | "restart" | "close" => {
            nas_app_manager::control(&db, id, &action).await.map(Json)
        }
        _ => Err(ApiFailure::bad("invalid application action")),
    }
}
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct Credentials {
    configured: bool,
    packages_enabled: bool,
    apps_managed: bool,
}

/// Whether the release token is set and what the launcher may do with packages
#[utoipa::path(get, path = "/apps/credentials", responses((status = OK, body = Credentials)))]
async fn credentials_state() -> Json<Credentials> {
    Json(Credentials {
        configured: credentials::launcher().await.is_some(),
        packages_enabled: utils::is_real(Mode::Packages),
        apps_managed: utils::is_real(Mode::Apps),
    })
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct NewToken {
    token: String,
}

#[derive(Serialize, ToSchema)]
struct Accepted {
    accepted: bool,
}

/// Stores the release token without ever returning it
#[utoipa::path(
    put,
    path = "/apps/credentials",
    request_body = NewToken,
    responses((status = OK, body = Accepted))
)]
async fn store_token(ValidJson(input): ValidJson<NewToken>) -> Result<Json<Accepted>, ApiFailure> {
    credentials::store(&input.token).await?;
    Ok(Json(Accepted { accepted: true }))
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(installed))
        .routes(routes!(installed_apps))
        .routes(routes!(available))
        .routes(routes!(catalogue_apps, save_catalogue))
        .routes(routes!(settings, save_settings))
        .routes(routes!(check))
        .routes(routes!(credentials_state, store_token))
        .routes(routes!(info))
        .routes(routes!(icon))
        .routes(
            routes!(logo, store_logo, forget_logo)
                .layer(DefaultBodyLimit::max(logos::MAX_BYTES * 4)),
        )
        .routes(routes!(close_all))
        .routes(routes!(action))
}
