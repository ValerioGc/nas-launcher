//! Bodies shared by several routes.

use serde::Serialize;
use utoipa::ToSchema;

/// Number of rows removed by a clearing operation.
#[derive(Serialize, ToSchema)]
pub struct Removed {
    pub removed: u64,
}
