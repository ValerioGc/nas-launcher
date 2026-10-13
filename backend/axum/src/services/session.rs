use crate::apps::AppsState;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct Session(pub Arc<RwLock<SessionData>>);

#[derive(Default)]
pub struct SessionData {
    pub apps: AppsState,
}
