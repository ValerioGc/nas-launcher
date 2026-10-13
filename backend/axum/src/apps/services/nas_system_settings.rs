pub mod access;
pub mod config;
pub mod discovery;
pub mod network;
pub mod power;
pub mod reset;
pub mod system_updates;
pub mod updates;

use crate::{
    http::error::ApiFailure,
    services::{
        database,
        settings::{self, Section},
    },
};
use power::schedule;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::time::Duration;

use crate::utils::{self, Mode};

pub use updates::NasUpdates;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    System,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    It,
    En,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundMode {
    Image,
    Gradient,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundPattern {
    Halo,
    Dots,
    Multiple,
    Linear,
    Rings,
    Grid,
    Stripes,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum PowerDay {
    Everyday,
    Weekdays,
    Weekends,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum PowerAction {
    Start,
    Shutdown,
    Restart,
}

/// How big the icons of the desktop are drawn: `large` is the size they always had.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum DesktopIconSize {
    Small,
    Medium,
    #[default]
    Large,
    #[serde(rename = "xlarge")]
    ExtraLarge,
}

#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Background {
    pub mode: BackgroundMode,
    pub image: String,
    pub blur: u8,
    pub intensity: u8,
    pub color: String,
    pub direction: u16,
    pub pattern: BackgroundPattern,
    pub size: u8,
}
/// The theme chosen for each type, by the id of its file in `src/styles/themes/`: the interface
/// draws the default for an id it does not know.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Themes {
    #[serde(default = "default_theme")]
    pub light: String,
    #[serde(default = "default_theme")]
    pub dark: String,
}

impl Default for Themes {
    fn default() -> Self {
        Self {
            light: default_theme(),
            dark: default_theme(),
        }
    }
}

fn default_theme() -> String {
    "default".to_owned()
}

/// An id of a theme: lower-case letters and hyphens, as the name of its file.
pub fn is_theme_id(id: &str) -> bool {
    (1..=32).contains(&id.len())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
}

#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct General {
    #[serde(default = "default_nas_name")]
    pub nas_name: String,
    pub theme: Theme,
    /// The theme of each type, for the type `theme` resolves to.
    #[serde(default)]
    pub themes: Themes,
    pub language: Language,
    pub create_shortcut: bool,
    #[serde(default)]
    pub desktop_icon_size: DesktopIconSize,
    #[serde(default)]
    pub accent: String,
    pub background: Background,
}

/// Allowlist of wallpaper paths accepted from the UI.
const WALLPAPERS: [&str; 6] = [
    "/wallpapers/landscape.svg",
    "/wallpapers/waves.svg",
    "/wallpapers/aurora.svg",
    "/wallpapers/shapes.svg",
    "/wallpapers/topography.svg",
    "/wallpapers/mesh.svg",
];

fn default_nas_name() -> String {
    "nas".to_owned()
}

pub fn is_colour(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

#[derive(Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Access {
    pub notify_failures: bool,
}
#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PowerRule {
    pub day: PowerDay,
    pub time: String,
    pub action: PowerAction,
    pub active: bool,
}

#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Power {
    pub enabled: bool,
    pub rules: Vec<PowerRule>,
    /// The minutes without a key or the mouse after which the monitor connected to the NAS goes
    /// dark and into standby, from 1 to 240.
    #[serde(rename = "screenOffMinutes", default = "screen_off_default")]
    pub screen_off_minutes: u16,
}

pub const SCREEN_OFF_DEFAULT: u16 = 10;
const SCREEN_OFF_MAX: u16 = 240;

fn screen_off_default() -> u16 {
    SCREEN_OFF_DEFAULT
}

impl Default for Power {
    fn default() -> Self {
        Self {
            enabled: false,
            rules: Vec::new(),
            screen_off_minutes: SCREEN_OFF_DEFAULT,
        }
    }
}

impl Default for General {
    fn default() -> Self {
        Self {
            nas_name: default_nas_name(),
            theme: Theme::System,
            themes: Themes::default(),
            language: Language::It,
            create_shortcut: true,
            desktop_icon_size: DesktopIconSize::Large,
            accent: String::new(),
            background: Background {
                mode: BackgroundMode::Gradient,
                image: "/wallpapers/landscape.svg".into(),
                blur: 0,
                intensity: 35,
                color: "#258ac0".into(),
                direction: 135,
                pattern: BackgroundPattern::Multiple,
                size: 60,
            },
        }
    }
}
impl General {
    pub fn validate(&self) -> Result<(), ApiFailure> {
        let b = &self.background;
        let valid_nas_name = self.nas_name.len() <= 63
            && self
                .nas_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && self
                .nas_name
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            && self
                .nas_name
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric);
        if !valid_nas_name
            || b.blur > 30
            || b.intensity > 100
            || b.direction > 360
            || b.size < 10
            || b.size > 100
            || !is_colour(&b.color)
            || !WALLPAPERS.contains(&b.image.as_str())
            || !(self.accent.is_empty() || is_colour(&self.accent))
            || !is_theme_id(&self.themes.light)
            || !is_theme_id(&self.themes.dark)
        {
            return Err(ApiFailure::bad("invalid general settings"));
        }
        Ok(())
    }
}
impl Power {
    pub fn validate(&self) -> Result<(), ApiFailure> {
        if self.rules.len() > 32
            || !(1..=SCREEN_OFF_MAX).contains(&self.screen_off_minutes)
            || self.rules.iter().any(|r| {
                !r.time.split_once(':').is_some_and(|(h, m)| {
                    h.len() == 2
                        && m.len() == 2
                        && h.parse::<u8>().is_ok_and(|h| h < 24)
                        && m.parse::<u8>().is_ok_and(|m| m < 60)
                })
            })
        {
            return Err(ApiFailure::bad("invalid power schedule"));
        }
        Ok(())
    }
}

pub async fn load_general(db: &SqlitePool) -> Result<General, ApiFailure> {
    settings::read(db, Section::General).await
}

pub async fn load_access(db: &SqlitePool) -> Result<Access, ApiFailure> {
    settings::read(db, Section::Access).await
}

pub use discovery::{load as load_discovery, save as save_discovery, Discovery};

pub async fn load_power(db: &SqlitePool) -> Result<Power, ApiFailure> {
    settings::read(db, Section::Power).await
}

/// The name of the NAS as the hostname of the machine, through `nas-os-hostname.service`, where
/// `NAS_HOSTNAME_MODE` is real. The name is checked already.
pub async fn apply_hostname(name: &str) -> Result<(), ApiFailure> {
    if !utils::is_real(Mode::Hostname) {
        return Ok(());
    }
    tokio::fs::write("/var/lib/nas-os/hostname", name.to_ascii_lowercase())
        .await
        .map_err(|_| ApiFailure::unavailable("cannot prepare the NAS hostname"))?;
    utils::systemctl(
        &["start", "nas-os-hostname.service"],
        Duration::from_secs(20),
    )
    .await
    .map_err(|_| ApiFailure::unavailable("cannot apply the NAS hostname"))?;
    if let Ok(reason) = tokio::fs::read_to_string("/var/lib/nas-os/hostname.error").await {
        let reason = reason.trim();
        return Err(ApiFailure::unavailable(if reason.is_empty() {
            "hostname update failed"
        } else {
            reason
        }));
    }
    Ok(())
}

pub async fn save_general(db: &SqlitePool, input: General) -> Result<General, ApiFailure> {
    input.validate()?;
    apply_hostname(&input.nas_name).await?;
    settings::write(db, Section::General, &input).await?;
    settings::record_saved(db, Section::General).await;
    Ok(input)
}

pub async fn save_access(db: &SqlitePool, input: Access) -> Result<Access, ApiFailure> {
    settings::write(db, Section::Access, &input).await?;
    settings::record_saved(db, Section::Access).await;
    Ok(input)
}

/// Hands the minutes of the screen to `nas-os-screen.service`, which reads them beside the
/// database. A screen that cannot read them goes dark after the default.
pub async fn apply_screen(minutes: u16) {
    let Some(path) = database::beside("screen-off-minutes") else {
        return;
    };
    if let Err(error) = tokio::fs::write(&path, format!("{minutes}\n")).await {
        eprintln!("the minutes of the screen were not written: {error}");
    }
}

pub async fn save_power(db: &SqlitePool, input: Power) -> Result<Power, ApiFailure> {
    input.validate()?;
    settings::write(db, Section::Power, &input).await?;
    apply_screen(input.screen_off_minutes).await;
    schedule::rules_changed();
    settings::record_saved(db, Section::Power).await;
    Ok(input)
}
