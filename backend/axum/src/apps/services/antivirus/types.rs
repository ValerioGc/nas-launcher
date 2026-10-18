use crate::http::error::ApiFailure;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ScanKind {
    Full,
    Quick,
    File,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ScanOptions {
    pub archives: bool,
    pub detect_pua: bool,
    pub max_file_mb: u32,
    pub paths: Vec<String>,
    pub exclusions: Vec<String>,
}
impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            archives: true,
            detect_pua: false,
            max_file_mb: 100,
            paths: Vec::new(),
            exclusions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Schedule {
    pub id: String,
    pub enabled: bool,
    pub kind: ScanKind,
    /// systemd day name, or * for every day. Time uses the NAS timezone.
    pub day: String,
    pub time: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LifecycleScan {
    pub enabled: bool,
    pub kind: ScanKind,
    pub paths: Vec<String>,
}
impl Default for LifecycleScan {
    fn default() -> Self {
        Self {
            enabled: false,
            kind: ScanKind::Quick,
            paths: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Settings {
    pub enabled: bool,
    pub scheduler_enabled: bool,
    #[serde(default)]
    pub boot_scan: LifecycleScan,
    #[serde(default)]
    pub shutdown_scan: LifecycleScan,
    pub auto_quarantine: bool,
    pub full: ScanOptions,
    pub quick: ScanOptions,
    pub file: ScanOptions,
    pub custom: ScanOptions,
    pub schedules: Vec<Schedule>,
}
impl Settings {
    pub fn options(&self, kind: ScanKind) -> &ScanOptions {
        match kind {
            ScanKind::Full => &self.full,
            ScanKind::Quick => &self.quick,
            ScanKind::File => &self.file,
            ScanKind::Custom => &self.custom,
        }
    }
    pub fn validate(&self) -> Result<(), ApiFailure> {
        for scan in [&self.boot_scan, &self.shutdown_scan] {
            if scan.enabled {
                ScanRequest {
                    kind: scan.kind,
                    paths: scan.paths.clone(),
                    skip_unchanged: false,
                }
                .validate()?;
            } else if !valid_paths(&scan.paths) {
                return Err(ApiFailure::bad("invalid lifecycle scan paths"));
            }
        }
        for options in [&self.full, &self.quick, &self.file, &self.custom] {
            if !(1..=2048).contains(&options.max_file_mb)
                || !valid_paths(&options.paths)
                || !valid_paths(&options.exclusions)
            {
                return Err(ApiFailure::bad("invalid scan options"));
            }
        }
        if self.schedules.len() > 20 {
            return Err(ApiFailure::bad("too many antivirus jobs"));
        }
        let mut ids = std::collections::HashSet::new();
        for job in &self.schedules {
            if !identifier(&job.id)
                || !ids.insert(&job.id)
                || !["*", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
                    .contains(&job.day.as_str())
                || !valid_time(&job.time)
                || !valid_paths(&job.paths)
                || (matches!(job.kind, ScanKind::File | ScanKind::Custom) && job.paths.is_empty())
                || (job.kind == ScanKind::File && job.paths.len() != 1)
            {
                return Err(ApiFailure::bad("invalid antivirus job"));
            }
        }
        Ok(())
    }
}

pub fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}
pub fn valid_path(value: &str) -> bool {
    value.starts_with('/')
        && value.len() <= 4096
        && !value.chars().any(char::is_control)
        && !Path::new(value)
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
}
pub fn valid_paths(values: &[String]) -> bool {
    values.len() <= 100 && values.iter().all(|value| valid_path(value))
}
fn valid_time(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 5
        && bytes[2] == b':'
        && [0, 1, 3, 4]
            .iter()
            .all(|index| bytes[*index].is_ascii_digit())
        && value[..2].parse::<u8>().is_ok_and(|hour| hour < 24)
        && value[3..].parse::<u8>().is_ok_and(|minute| minute < 60)
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ScanRequest {
    pub kind: ScanKind,
    pub paths: Vec<String>,
    /// Enables incremental clean-file filtering for custom scans, as always used by quick
    /// scans.
    #[serde(default, rename = "skipUnchanged")]
    pub skip_unchanged: bool,
}
impl ScanRequest {
    pub fn validate(&self) -> Result<(), ApiFailure> {
        if !valid_paths(&self.paths)
            || (matches!(self.kind, ScanKind::File | ScanKind::Custom) && self.paths.is_empty())
            || (self.kind == ScanKind::File && self.paths.len() != 1)
        {
            return Err(ApiFailure::bad("invalid antivirus scan paths"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Detection {
    pub id: String,
    pub path: String,
    pub threat: String,
    pub device: u64,
    pub inode: u64,
    pub options: ScanOptions,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuarantineEntry {
    pub detection: Detection,
    pub at: i64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub at: i64,
    pub updated_at: i64,
    pub kind: Option<ScanKind>,
    pub scanned: u64,
    pub found: u64,
    /// Files left out because they were found clean and did not change since.
    #[serde(default)]
    pub skipped: u64,
    /// Files sent to ClamAV, when the scan knows it beforehand; 0 when it does not.
    #[serde(default)]
    pub planned: u64,
    /// The files the archive of the scans holds as clean, and the room it takes on the disk.
    #[serde(default)]
    pub cache_files: u64,
    #[serde(default)]
    pub cache_bytes: u64,
    pub detections: Vec<Detection>,
    pub quarantine: Vec<QuarantineEntry>,
    pub error: Option<String>,
    pub operation_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase", deny_unknown_fields)]
pub enum Job {
    Configure {
        settings: Box<Settings>,
    },
    Enable,
    Disable,
    Resume {
        enabled: bool,
    },
    Scan {
        request: ScanRequest,
        options: ScanOptions,
        auto_quarantine: bool,
    },
    Quarantine {
        id: String,
    },
    Restore {
        id: String,
    },
    Delete {
        id: String,
    },
    /// Clears cached clean results so the next quick scan checks every file.
    Forget,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Toggle {
    pub enabled: bool,
}
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EntryRequest {
    pub id: String,
}

#[derive(Debug, Clone, Default, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub phase: String,
    pub progress: Option<f64>,
    pub error: Option<String>,
}
#[derive(Serialize, ToSchema)]
pub struct Status {
    pub settings: Settings,
    pub operation: Operation,
    pub report: Report,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleNotice {
    pub at: i64,
    pub found: u64,
    pub scanned: u64,
    pub error: Option<String>,
}
