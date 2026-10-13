//! Shared HTTP routing, state, extractors and JSON errors.

pub mod app_channel;
pub mod boundary;
pub mod error;
pub mod extract;
pub mod guard;
pub mod openapi;
pub mod proxy;
pub mod reply;
pub mod routes;
pub mod state;

pub use error::{ApiError, ApiFailure};
pub use extract::ValidJson;
pub use routes::build_router;
pub use state::AppState;
