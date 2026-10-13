//! Bodies shared by several routes.

use serde::Serialize;
use utoipa::ToSchema;

/// How many rows a clearing route deleted.
#[derive(Serialize, ToSchema)]
pub struct Removed {
    pub removed: u64,
}
