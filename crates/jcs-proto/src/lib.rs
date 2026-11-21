//! Wire protocol between the `jcs` client and the `jcsd` daemon.
//!
//! Versioned explicitly: the daemon refuses requests from a different major protocol
//! version instead of misinterpreting them. All types are plain JSON; the event stream
//! is newline-delimited JSON with the same [`EventDto`] shape as the REST responses.

pub use jiff::Timestamp;
use serde::{Deserialize, Serialize};

pub mod paths;

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
    pub created_at: Timestamp,
    /// Whether succeeded tasks are merged into `base_branch` automatically.
    pub auto_merge: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_usd_per_day: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tokens_per_day: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddProjectRequest {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
}

/// Full replacement of a project's execution settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub auto_merge: bool,
    #[serde(default)]
    pub budget_usd_per_day: Option<f64>,
    #[serde(default)]
    pub budget_tokens_per_day: Option<i64>,
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
    /// Title override for the created task (only used with `plan = false`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Decompose the objective into a task DAG. On by default.
    #[serde(default = "default_true")]
    pub plan: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskResponse {
    pub goal_id: String,
    /// Whether the daemon will plan the goal before executing it.
    pub planned: bool,
    /// Present only for a single-task (`plan = false`) objective.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalDetailResponse {
    pub goal: GoalView,
    pub tasks: Vec<TaskView>,
    pub runs: Vec<RunView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalActionResponse {
    pub ok: bool,
    pub goal_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalView {
    pub id: String,
    pub project_id: String,
    pub prompt: String,
    pub status: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
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
    /// Task ids this task waits for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunView {
    pub id: String,
    pub task_id: String,
    pub attempt: u32,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub started_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<Timestamp>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyPackView {
    pub id: String,
    pub version: String,
    pub hash: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRuleView {
    pub key: String,
    pub value: String,
    /// Pack id that defined the rule, or `project`.
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyGateView {
    pub name: String,
    pub command: Vec<String>,
    pub timeout_secs: u64,
    pub required: bool,
    pub parser: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyView {
    pub project: String,
    pub packs: Vec<PolicyPackView>,
    pub rules: Vec<PolicyRuleView>,
    pub gates: Vec<PolicyGateView>,
    pub skills: Vec<String>,
    /// Rendered files that differ from the policy, or are missing.
    pub drift: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackCatalogEntry {
    pub id: String,
    pub version: String,
    pub description: String,
    pub gates: usize,
    pub requires: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderPolicyRequest {
    /// Project name or path.
    pub project: String,
    #[serde(default)]
    pub check: bool,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderedFileView {
    pub path: String,
    /// created | updated | unchanged | drifted | missing | refused
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderPolicyResponse {
    pub ok: bool,
    pub check: bool,
    pub files: Vec<RenderedFileView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateRunRequest {
    pub project: String,
    /// Run only this gate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateOutcomeView {
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub duration_ms: i64,
    pub excerpt: String,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateRunResponse {
    pub ok: bool,
    pub outcomes: Vec<GateOutcomeView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryIndexRequest {
    pub project: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryIndexReportView {
    pub files_seen: usize,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub files_removed: usize,
    pub symbols: usize,
    pub chunks: usize,
    pub embeddings: usize,
    pub duration_ms: i64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryIndexResponse {
    pub report: MemoryIndexReportView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryHitView {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    pub score: f32,
    pub source: String,
    pub citation: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResponse {
    pub query: String,
    pub hits: Vec<MemoryHitView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatsView {
    pub project: String,
    pub files: i64,
    pub symbols: i64,
    pub chunks: i64,
    pub embedded_chunks: i64,
    pub edges: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_indexed_at: Option<String>,
    pub semantic_enabled: bool,
}

/// A model the backend advertises, with its price per million tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCatalogEntry {
    /// `provider/model`, accepted anywhere a model reference is expected.
    pub reference: String,
    pub name: String,
    pub cost_input: f64,
    pub cost_output: f64,
}

/// Effective daemon settings, with the ones that need a restart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsView {
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub default_agent: Option<String>,
    #[serde(default)]
    pub planning_model: Option<String>,
    #[serde(default)]
    pub planning_agent: Option<String>,
    #[serde(default)]
    pub review_model: Option<String>,
    pub review: bool,
    pub risk_max_lines: u64,
    pub run_timeout_secs: u64,
    pub context_tokens: u64,
    pub max_repair_cycles: u64,
    pub max_concurrent: u64,
    /// Settings changed at runtime whose new value only applies after restarting `jcsd`.
    #[serde(default)]
    pub restart_required: Vec<String>,
}

/// Partial update. A missing field is unchanged; an empty string clears a model or agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SettingsPatch {
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub default_agent: Option<String>,
    #[serde(default)]
    pub planning_model: Option<String>,
    #[serde(default)]
    pub planning_agent: Option<String>,
    #[serde(default)]
    pub review_model: Option<String>,
    #[serde(default)]
    pub review: Option<bool>,
    #[serde(default)]
    pub risk_max_lines: Option<u64>,
    #[serde(default)]
    pub run_timeout_secs: Option<u64>,
    #[serde(default)]
    pub context_tokens: Option<u64>,
    #[serde(default)]
    pub max_repair_cycles: Option<u64>,
    #[serde(default)]
    pub max_concurrent: Option<u64>,
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
    fn settings_patch_is_partial() {
        let patch: SettingsPatch = serde_json::from_str(r#"{"review":true}"#).unwrap();
        assert_eq!(patch.review, Some(true));
        assert!(patch.default_model.is_none());
        assert!(patch.run_timeout_secs.is_none());
    }

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
        let body = ErrorBody::new(
            "project_not_found",
            "no project named demo; run `jcs project add`",
        );
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("project_not_found"));
        assert!(json.contains("jcs project add"));
    }
}
