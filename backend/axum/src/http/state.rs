//! Handler state, extracted independently as a session or a SQLite pool.

use crate::services::session::Session;
use axum::extract::FromRef;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub session: Session,
    pub db: SqlitePool,
}

impl AppState {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            session: Session::default(),
            db,
        }
    }
}

impl FromRef<AppState> for Session {
    fn from_ref(state: &AppState) -> Self {
        state.session.clone()
    }
}

impl FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.db.clone()
    }
}
