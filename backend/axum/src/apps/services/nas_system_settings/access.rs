//! Access rules stored in SQLite and enforced by HTTP middleware and, with
//! `NAS_FIREWALL_MODE=real`, the system firewall.

use crate::{
    http::error::ApiFailure,
    services::{
        database, mail_events,
        notifications::{self, Notice, Tone},
        settings::{self, Section},
    },
    utils::{self, now_ms, Mode},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{net::IpAddr, time::Duration};

const FIREWALL_UNIT: &str = "nas-os-firewall.service";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum RuleKind {
    Ip,
    Range,
}
impl RuleKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Ip => "ip",
            Self::Range => "range",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "ip" => Some(Self::Ip),
            "range" => Some(Self::Range),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    pub kind: RuleKind,
    pub value: String,
    pub label: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewRule {
    pub kind: RuleKind,
    pub value: String,
    #[serde(default)]
    pub label: String,
}

pub fn normalise(kind: RuleKind, value: &str) -> Result<String, ApiFailure> {
    let value = value.trim();
    match kind {
        RuleKind::Ip => value
            .parse::<IpAddr>()
            .map(|address| address.to_string())
            .map_err(|_| ApiFailure::bad("invalid IP address")),
        RuleKind::Range => {
            let (address, prefix) = split_range(value)?;
            Ok(format!("{address}/{prefix}"))
        }
    }
}

fn split_range(value: &str) -> Result<(IpAddr, u8), ApiFailure> {
    let invalid = || ApiFailure::bad("invalid address range");
    let (address, prefix) = value.split_once('/').ok_or_else(invalid)?;
    let address: IpAddr = address.trim().parse().map_err(|_| invalid())?;
    let prefix: u8 = prefix.trim().parse().map_err(|_| invalid())?;
    let bits = if address.is_ipv4() { 32 } else { 128 };
    if prefix > bits {
        return Err(invalid());
    }
    Ok((address, prefix))
}

fn within(address: IpAddr, network: IpAddr, prefix: u8) -> bool {
    match (address, network) {
        (IpAddr::V4(address), IpAddr::V4(network)) => {
            shares_prefix(&address.octets(), &network.octets(), prefix)
        }
        (IpAddr::V6(address), IpAddr::V6(network)) => {
            shares_prefix(&address.octets(), &network.octets(), prefix)
        }
        _ => false,
    }
}

fn shares_prefix(address: &[u8], network: &[u8], prefix: u8) -> bool {
    let whole = usize::from(prefix / 8);
    if address[..whole] != network[..whole] {
        return false;
    }
    let remaining = prefix % 8;
    if remaining == 0 {
        return true;
    }
    let mask = 0xffu8 << (8 - remaining);
    address[whole] & mask == network[whole] & mask
}

pub async fn list(db: &SqlitePool) -> Result<Vec<Rule>, ApiFailure> {
    let rows: Vec<(i64, String, String, String, i64)> = sqlx::query_as(
        "SELECT id, kind, value, label, created_at FROM access_rules ORDER BY created_at, id",
    )
    .fetch_all(db)
    .await
    .map_err(|_| unavailable())?;

    Ok(rows
        .into_iter()
        .map(|(id, kind, value, label, created_at)| Rule {
            id: id.to_string(),
            kind: RuleKind::parse(&kind).unwrap_or(RuleKind::Ip),
            value,
            label,
            created_at,
        })
        .collect())
}

/// The value and the label of a rule as they are stored, or why the rule is refused.
pub fn checked(input: &NewRule) -> Result<(String, &str), ApiFailure> {
    let value = normalise(input.kind, &input.value)?;
    let label = input.label.trim();
    if label.len() > 64 {
        return Err(ApiFailure::bad("the label of this rule is too long"));
    }
    Ok((value, label))
}

pub async fn add(db: &SqlitePool, input: NewRule) -> Result<Vec<Rule>, ApiFailure> {
    let (value, label) = checked(&input)?;

    sqlx::query(
        "INSERT INTO access_rules (kind, value, label, created_at) VALUES (?, ?, ?, ?)
         ON CONFLICT (kind, value) DO UPDATE SET label = excluded.label",
    )
    .bind(input.kind.id())
    .bind(&value)
    .bind(label)
    .bind(now_ms())
    .execute(db)
    .await
    .map_err(|_| unavailable())?;

    let rules = list(db).await?;
    applied(db, &rules, true, &value).await;
    Ok(rules)
}

pub async fn remove(db: &SqlitePool, id: &str) -> Result<Vec<Rule>, ApiFailure> {
    let key: i64 = id.parse().map_err(|_| ApiFailure::missing("access rule"))?;
    let removed: Option<(String,)> =
        sqlx::query_as("DELETE FROM access_rules WHERE id = ? RETURNING value")
            .bind(key)
            .fetch_optional(db)
            .await
            .map_err(|_| unavailable())?;
    let Some((value,)) = removed else {
        return Err(ApiFailure::missing("access rule"));
    };

    let rules = list(db).await?;
    applied(db, &rules, false, &value).await;
    Ok(rules)
}

pub async fn allows(db: &SqlitePool, address: IpAddr) -> bool {
    if address.is_loopback() {
        return true;
    }

    let Ok(rules) = list(db).await else {
        return true;
    };

    if rules.is_empty() {
        return true;
    }

    rules.iter().any(|rule| match rule.kind {
        RuleKind::Ip => rule
            .value
            .parse::<IpAddr>()
            .is_ok_and(|rule| rule == address),
        RuleKind::Range => {
            split_range(&rule.value).is_ok_and(|(network, prefix)| within(address, network, prefix))
        }
    })
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccessPreferences {
    #[serde(default)]
    notify_failures: bool,
}

/// Dispatches sign-in failures to the mail application and optionally records a notification.
pub async fn record_failure(db: &SqlitePool, address: Option<IpAddr>, field: &str) -> bool {
    let from = address.map_or_else(
        || "an unknown address".to_owned(),
        |address| address.to_string(),
    );
    let notice = Notice::new(
        "signInRefused",
        "Sign-in refused",
        format!("Sign-in attempt from {from} refused for invalid credentials ({field})."),
    )
    .with("from", from)
    .with("field", field);
    mail_events::forward(&notice);

    let wanted = settings::read::<AccessPreferences>(db, Section::Access)
        .await
        .map(|preferences| preferences.notify_failures)
        .unwrap_or_default();
    if wanted {
        notifications::record(db, "access", Tone::Warning, notice).await;
    }
    wanted
}

fn request_path() -> Option<std::path::PathBuf> {
    database::beside("access-rules")
}

pub async fn apply(rules: &[Rule]) -> Result<(), String> {
    if !utils::is_real(Mode::Firewall) {
        return Ok(());
    }

    let path = request_path().ok_or("no state directory for the access rules")?;
    let payload = rules
        .iter()
        .map(|rule| rule.value.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    tokio::fs::write(&path, payload)
        .await
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;

    utils::systemctl(&["start", FIREWALL_UNIT], COMMAND_TIMEOUT).await
}

pub async fn apply_saved(db: &SqlitePool) {
    if !utils::is_real(Mode::Firewall) {
        return;
    }

    let Ok(rules) = list(db).await else {
        eprintln!("access rules not applied at startup: the list cannot be read");
        return;
    };

    if let Err(reason) = apply(&rules).await {
        not_applied(db, reason).await;
    }
}

async fn not_applied(db: &SqlitePool, reason: String) {
    notifications::record(
        db,
        "access",
        Tone::Error,
        Notice::new(
            "accessRulesFailed",
            "Access rules not applied",
            format!(
                "Allowed addresses saved but not yet applied to the network filter ({reason})."
            ),
        )
        .with("reason", reason.clone())
        .detail("access-rules", &reason, None)
        .await,
    )
    .await;
}

async fn applied(db: &SqlitePool, rules: &[Rule], added: bool, value: &str) {
    match apply(rules).await {
        Ok(()) => {
            let notice = if added {
                Notice::new(
                    "accessRuleAdded",
                    "Address allowed",
                    format!("Address {value} added to the allowed addresses."),
                )
            } else {
                Notice::new(
                    "accessRuleRemoved",
                    "Address removed",
                    format!("Address {value} removed from the allowed addresses."),
                )
            };
            notifications::record(db, "access", Tone::Info, notice.with("value", value)).await;
        }
        Err(reason) => not_applied(db, reason).await,
    }
}

fn unavailable() -> ApiFailure {
    ApiFailure::unavailable("the access rules cannot be read")
}

#[cfg(test)]
#[path = "../../../../tests/apps/services/nas_system_settings/access.rs"]
mod tests;
