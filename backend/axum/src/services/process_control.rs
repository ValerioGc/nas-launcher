//! Privileged process control shared by the shell integration. No monitoring or sensor collection.
//!
//! The launcher has no capability: it cannot signal the processes of others nor restart their
//! units. It starts `nas-os-process@<verb>-<pid>-<started>.service`, the only unit polkit lets it
//! start for this (`65-nas-os-processes.rules`), whose helper `act-on-process.sh` does the work as
//! root after checking the same rules again. The start time travels with the pid, so a process that
//! ended and left its id to another one is never the one acted on.
//!
//! What keeps the NAS working, [`ESSENTIAL`], may only be restarted, never terminated. A thread of
//! the kernel can be neither; restarting needs a service, which a login session or a scope is not.

use crate::{
    http::error::ApiFailure,
    services::{lifecycle, manifest},
    utils::{self, Mode},
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// The units whose processes are only restarted, never terminated, as shell patterns: the same list
/// is in `act-on-process.sh`, which decides.
pub const ESSENTIAL: [&str; 9] = [
    "init.scope",
    "nas-os.service",
    "nas-os-gateway.service",
    "dbus.service",
    "dbus-broker.service",
    "polkit.service",
    "systemd-*.service",
    "NetworkManager.service",
    "ssh.service",
];

/// The unit of the helper: restarting it would stop the work it is doing.
const HELPER: &str = "nas-os-process@";
/// The helper waits for a restart to finish, which a slow service makes long.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Terminate,
    Restart,
}

impl Action {
    fn verb(self) -> &'static str {
        match self {
            Self::Terminate => "terminate",
            Self::Restart => "restart",
        }
    }
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProcessRequest {
    pub action: Action,
    /// The start time accompanying the process identifier.
    pub started: u64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProcessResult {
    pub app_id: Option<String>,
}

fn terminated_app(action: Action, unit: Option<&str>) -> Option<String> {
    if action != Action::Terminate {
        return None;
    }
    let id = unit?.strip_prefix("nas-app@")?.strip_suffix(".service")?;
    manifest::is_identifier(id).then(|| id.to_owned())
}

/// What may be done to a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allowed {
    pub terminate: bool,
    pub restart: bool,
}

/// Whether `unit` matches the shell pattern `pattern`, where `*` stands for any text.
fn matches(pattern: &str, unit: &str) -> bool {
    match pattern.split_once('*') {
        Some((head, tail)) => {
            unit.len() >= head.len() + tail.len() && unit.starts_with(head) && unit.ends_with(tail)
        }
        None => pattern == unit,
    }
}

pub fn is_essential(unit: &str) -> bool {
    ESSENTIAL.iter().any(|pattern| matches(pattern, unit))
}

/// The unit of a process from the text of its `/proc/<pid>/cgroup`: the first part of its path that
/// is a service or a scope, so a process of an application's own subgroup still belongs to it.
pub fn unit_in(cgroup: &str) -> Option<String> {
    let path = cgroup.lines().find_map(|line| line.strip_prefix("0::"))?;
    path.split('/')
        .find(|part| part.ends_with(".service") || part.ends_with(".scope"))
        .map(str::to_owned)
}

/// Authorizes an operation using current process metadata, independently of an application.
fn allowed(seen: &Seen, unit: Option<&str>) -> Allowed {
    if seen.kernel || seen.id == 1 {
        return Allowed {
            terminate: false,
            restart: false,
        };
    }
    Allowed {
        terminate: !unit.is_some_and(is_essential),
        restart: unit.is_some_and(|unit| unit.ends_with(".service") && !unit.starts_with(HELPER)),
    }
}

/// The unit of the helper that does `action` to the process `id` started at `started`.
pub fn helper_unit(action: Action, id: u32, started: u64) -> String {
    format!("{HELPER}{}-{id}-{started}.service", action.verb())
}

/// Metadata needed to authorize a privileged process operation.
struct Seen {
    id: u32,
    name: String,
    started: u64,
    kernel: bool,
}

fn seen(id: u32, stat: &str) -> Option<Seen> {
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let fields: Vec<&str> = stat.get(close + 1..)?.split_whitespace().collect();
    let parent: u32 = fields.get(1)?.parse().ok()?;
    Some(Seen {
        id,
        name: stat.get(open + 1..close)?.to_owned(),
        started: fields.get(19)?.parse().ok()?,
        kernel: id == 2 || parent == 2,
    })
}

/// Checks the request against the process as it is now, then has the helper do it.
pub async fn act(id: u32, request: ProcessRequest) -> Result<ProcessResult, ApiFailure> {
    let gone = || ApiFailure::missing(format!("process {id} is no longer running"));
    let stat = tokio::fs::read_to_string(format!("/proc/{id}/stat"))
        .await
        .map_err(|_| gone())?;
    // Only the start time and the kind are read here: the size of a page does not matter.
    let seen = seen(id, &stat).ok_or_else(gone)?;
    if seen.started != request.started {
        return Err(gone());
    }
    let unit = tokio::fs::read_to_string(format!("/proc/{id}/cgroup"))
        .await
        .ok()
        .and_then(|text| unit_in(&text));
    let allowed = allowed(&seen, unit.as_deref());
    match request.action {
        Action::Terminate if !allowed.terminate => {
            return Err(ApiFailure::forbidden(format!(
                "{} keeps the NAS working: it can only be restarted",
                unit.as_deref().unwrap_or(&seen.name)
            )));
        }
        Action::Restart if !allowed.restart => {
            return Err(ApiFailure::forbidden(format!(
                "{} is not a service that can be restarted",
                unit.as_deref().unwrap_or(&seen.name)
            )));
        }
        _ => {}
    }
    if !utils::is_real(Mode::Processes) {
        return Err(ApiFailure::unavailable(
            "Managing processes is not available on this installation.",
        ));
    }

    let helper = helper_unit(request.action, id, request.started);
    utils::systemctl(&["start", &helper], COMMAND_TIMEOUT)
        .await
        .map_err(|reason| {
            ApiFailure::unavailable(format!(
                "process {id} was not {}: {reason}",
                match request.action {
                    Action::Terminate => "terminated",
                    Action::Restart => "restarted",
                }
            ))
        })?;
    let app_id = terminated_app(request.action, unit.as_deref());
    if let Some(ref app) = app_id {
        let unit = lifecycle::unit_of(app).ok_or_else(|| ApiFailure::bad("invalid application"))?;
        lifecycle::wait_for_stop(&unit)
            .await
            .map_err(ApiFailure::unavailable)?;
    }
    Ok(ProcessResult { app_id })
}

#[cfg(test)]
#[path = "../../../tests/services/process_control.rs"]
mod tests;
