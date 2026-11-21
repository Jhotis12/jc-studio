//! Wire protocol between the `jcs` client and the `jcsd` daemon.
//!
//! Versioned explicitly: the daemon refuses requests from a different major protocol
//! version instead of misinterpreting them. All types are plain JSON; the event stream
//! is newline-delimited JSON with the same [`EventDto`] shape as the REST responses.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Bumped on breaking changes. Clients send it in `x-jcs-protocol`.
pub const PROTOCOL_VERSION: u32 = 1;

/// Header carrying [`PROTOCOL_VERSION`].
pub const PROTOCOL_HEADER: &str = "x-jcs-protocol";

/// Default socket file name inside the runtime directory.
pub const SOCKET_NAME: &str = "jcsd.sock";

/// Machine-readable failure body. Never contains stack traces or credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Stable, snake_case code for programmatic handling (e.g. `project_not_found`).
    pub code: String,
    /// Human-readable explanation with an actionable next step.
    pub message: String,
}

impl ErrorBody {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Health of the agent backend, as seen by the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHealth {
    pub backend: String,
    pub reachable: bool,
    /// Base URL without credentials. Safe to display.
    pub base_url: Option<String>,
    pub version: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub version: String,
    pub protocol: u32,
    pub uptime_secs: u64,
    pub database: String,
    pub agent: AgentHealth,
    /// Unix timestamp in milliseconds.
    pub started_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectView {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub base_branch: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddProjectRequest {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectResponse {
    pub project: ProjectView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectView>,
}

/// How the daemon should handle permission requests from the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    /// Deny anything not explicitly allowed by the rendered policy.
    Strict,
    /// Approve non-destructive actions automatically; still surface the rest.
    #[default]
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskRequest {
    /// Project name or absolute root path.
    pub project: String,
    pub prompt: String,
    /// Model reference as `provider/model`, e.g. `opencode/hy4-flash-free`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Agent name; the backend default when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default)]
    pub permissions: PermissionMode,
    /// Title override for the created task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskResponse {
    pub goal_id: String,
    pub task_id: String,
    pub run_id: String,
    pub task_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalView {
    pub id: String,
    pub project_id: String,
    pub prompt: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskView {
    pub id: String,
    pub goal_id: String,
    pub title: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunView {
    pub id: String,
    pub task_id: String,
    pub attempt: u32,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub started_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,
    pub cost_usd: f64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateView {
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Totals {
    pub projects: i64,
    pub goals: i64,
    pub tasks: i64,
    pub cost_usd: f64,
    pub tokens_in: i64,
    pub tokens_out: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub health: HealthResponse,
    pub projects: Vec<ProjectView>,
    pub goals: Vec<GoalView>,
    pub tasks: Vec<TaskView>,
    pub runs: Vec<RunView>,
    pub totals: Totals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunDetailResponse {
    pub run: RunView,
    pub gates: Vec<GateView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbortRequest {
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OkResponse {
    pub ok: bool,
}

/// An event as it appears on the client stream.
///
/// This is the domain event: one representation everywhere, so a client and the
/// database can never disagree about what happened.
pub type EventDto = jcs_core::Event;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_mode_defaults_to_auto() {
        assert_eq!(PermissionMode::default(), PermissionMode::Auto);
    }

    #[test]
    fn ask_request_is_forward_compatible() {
        let json = r#"{"project":"demo","prompt":"do it","future_field":42}"#;
        let parsed: AskRequest = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.project, "demo");
        assert_eq!(parsed.permissions, PermissionMode::Auto);
        assert!(parsed.model.is_none());
    }

    #[test]
    fn errors_carry_a_code_and_a_message() {
        let body = ErrorBody::new("project_not_found", "no project named demo; run `jcs project add`");
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("project_not_found"));
        assert!(json.contains("jcs project add"));
    }
}
