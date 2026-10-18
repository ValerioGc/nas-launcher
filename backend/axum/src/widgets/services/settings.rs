//! Polling settings for header widgets.

use crate::{
    http::error::ApiFailure,
    services::settings::{self, Section},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct WidgetSettings {
    pub polling_seconds: u16,
}
impl Default for WidgetSettings {
    fn default() -> Self {
        Self {
            polling_seconds: 10,
        }
    }
}
impl WidgetSettings {
    pub fn validate(&self) -> Result<(), ApiFailure> {
        if !(2..=300).contains(&self.polling_seconds) {
            return Err(ApiFailure::bad("polling must be between 2 and 300 seconds"));
        }
        Ok(())
    }
}

pub async fn load(db: &SqlitePool) -> Result<WidgetSettings, ApiFailure> {
    settings::read(db, Section::Widgets).await
}

pub async fn save(db: &SqlitePool, input: WidgetSettings) -> Result<WidgetSettings, ApiFailure> {
    input.validate()?;
    settings::write(db, Section::Widgets, &input).await?;
    settings::record_saved(db, Section::Widgets).await;
    Ok(input)
}
