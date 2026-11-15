//! Handler state, extracted independently as a session, a SQLite pool or the access rules.

use crate::{apps::services::nas_system_settings::access::AccessRules, services::session::Session};
use axum::extract::FromRef;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub session: Session,
    pub db: SqlitePool,
    pub access: AccessRules,
}

impl AppState {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            session: Session::default(),
            db,
            access: AccessRules::default(),
        }
    }
}

impl FromRef<AppState> for Session {
    fn from_ref(state: &AppState) -> Self {
        state.session.clone()
    }
}

impl FromRef<AppState> for AccessRules {
    fn from_ref(state: &AppState) -> Self {
        state.access.clone()
    }
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}
