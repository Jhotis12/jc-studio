use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::ids::{EventId, GoalId, ProjectId, RunId, TaskId};

/// Schema version of the persisted event payloads.
///
/// Bumping this is a migration: old rows stay readable because payloads are JSON
/// and readers tolerate unknown fields, but any *breaking* change to a typed payload
/// requires an upcaster in `jcs-store`.
pub const EVENT_SCHEMA_VERSION: u32 = 1;

/// The kind of fact that happened.
///
/// Event kinds are the vocabulary of the audit log and of the client stream. They are
/// stable strings: renaming one is a protocol change.
/// Declares the event vocabulary once. `as_str` and `parse` are generated from the
/// same table, so the database names can never drift from the in-memory names.
macro_rules! event_kinds {
    ($($variant:ident => $wire:literal),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum EventKind {
            $($variant),+
        }

        impl EventKind {
            /// Every kind, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }

            pub fn parse(value: &str) -> Result<Self, crate::CoreError> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    other => Err(crate::CoreError::unknown("EventKind", other)),
                }
            }
        }
    };
}

event_kinds! {
    // daemon lifecycle
    DaemonStarted => "daemon_started",
    DaemonStopping => "daemon_stopping",

    // projects
    ProjectAdded => "project_added",
    ProjectRemoved => "project_removed",

    // goals
    GoalCreated => "goal_created",
    GoalStatusChanged => "goal_status_changed",
    PlanCreated => "plan_created",
    GoalDeleted => "goal_deleted",

    // tasks
    TaskCreated => "task_created",
    TaskStatusChanged => "task_status_changed",
    TaskWorktreeCreated => "task_worktree_created",

    // runs
    RunStarted => "run_started",
    RunFinished => "run_finished",

    // agent activity
    AgentSessionLinked => "agent_session_linked",
    AgentStepStarted => "agent_step_started",
    AgentStepFinished => "agent_step_finished",
    // Chunks of assistant text are aggregated per step, never persisted per token.
    AgentText => "agent_text",
    // The model's reasoning stream, when the provider exposes it. Separate from the answer
    // so the interface can show it as "thinking" instead of mixing it with the output.
    AgentReasoning => "agent_reasoning",
    AgentToolCalled => "agent_tool_called",
    AgentToolFinished => "agent_tool_finished",
    AgentUsage => "agent_usage",

    // verification
    GateStarted => "gate_started",
    GateFinished => "gate_finished",

    // git
    CommitCreated => "commit_created",
    MergeFinished => "merge_finished",

    // policy and runtime configuration
    PolicyRendered => "policy_rendered",
    SettingsChanged => "settings_changed",

    // memory
    MemoryIndexed => "memory_indexed",

    // Project-level orchestrator chat. Text chunks are persisted as they stream; the
    // finished event carries the complete answer (or the error).
    OrchestratorText => "orchestrator_text",
    OrchestratorReasoning => "orchestrator_reasoning",
    OrchestratorFinished => "orchestrator_finished",

    // Something the operator should see. Not a crash log.
    Error => "error",
}

impl EventKind {
    /// Whether this kind must never be compacted away (audit-relevant).
    pub fn is_audit_permanent(self) -> bool {
        matches!(
            self,
            Self::RunStarted
                | Self::RunFinished
                | Self::GateStarted
                | Self::GateFinished
                | Self::CommitCreated
                | Self::MergeFinished
                | Self::GoalStatusChanged
                | Self::TaskStatusChanged
                | Self::GoalDeleted
                | Self::SettingsChanged
        )
    }
}

/// An event that has not been assigned a sequence number yet.
///
/// The store assigns `seq` transactionally with the state change it records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewEvent {
    pub id: EventId,
    pub ts: Timestamp,
    pub kind: EventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ProjectId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_id: Option<GoalId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    pub payload: serde_json::Value,
}

impl NewEvent {
    pub fn new(kind: EventKind) -> Self {
        Self {
            id: EventId::new(),
            ts: Timestamp::now(),
            kind,
            project_id: None,
            goal_id: None,
            task_id: None,
            run_id: None,
            payload: serde_json::Value::Null,
        }
    }

    pub fn at(mut self, ts: Timestamp) -> Self {
        self.ts = ts;
        self
    }

    pub fn project(mut self, id: &ProjectId) -> Self {
        self.project_id = Some(id.clone());
        self
    }

    pub fn goal(mut self, id: &GoalId) -> Self {
        self.goal_id = Some(id.clone());
        self
    }

    pub fn task(mut self, id: &TaskId) -> Self {
        self.task_id = Some(id.clone());
        self
    }

    pub fn run(mut self, id: &RunId) -> Self {
        self.run_id = Some(id.clone());
        self
    }

    /// Attaches a JSON payload. Serialization of domain values cannot fail for the
    /// types used here, so a failure is a programming error surfaced as `null`.
    pub fn payload<T: Serialize>(mut self, value: T) -> Self {
        self.payload = serde_json::to_value(value).unwrap_or(serde_json::Value::Null);
        self
    }

    /// Total ordering key for clients; assigned by the store on append.
    pub fn into_event(self, seq: i64) -> Event {
        Event {
            seq,
            id: self.id,
            ts: self.ts,
            kind: self.kind,
            project_id: self.project_id,
            goal_id: self.goal_id,
            task_id: self.task_id,
            run_id: self.run_id,
            payload: self.payload,
        }
    }
}

/// A persisted, ordered fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// Monotonic sequence assigned by SQLite. May contain gaps; never repeats.
    pub seq: i64,
    pub id: EventId,
    pub ts: Timestamp,
    pub kind: EventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ProjectId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_id: Option<GoalId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    pub payload: serde_json::Value,
}

impl Event {
    /// Canonical timestamp format used in the database: RFC 3339, millisecond
    /// precision, UTC. Fixed precision keeps string ordering equal to time ordering.
    pub fn encode_ts(ts: Timestamp) -> String {
        // Fixed millisecond precision, always UTC: lexicographic order in the database
        // must equal chronological order.
        ts.strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_wire_names_are_unique_and_match_serde() {
        let mut names: Vec<_> = EventKind::ALL.iter().map(|k| k.as_str()).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), before, "duplicate wire names");

        for kind in EventKind::ALL {
            // The serde rename and the explicit wire table must agree, otherwise the
            // database and the stream would disagree about an event's name.
            let serde_name = serde_json::to_string(kind).unwrap();
            assert_eq!(serde_name, format!("\"{}\"", kind.as_str()));
            assert_eq!(EventKind::parse(kind.as_str()).unwrap(), *kind);
        }
    }

    #[test]
    fn kind_parse_rejects_unknown_names() {
        assert!(EventKind::parse("task_created").is_ok());
        assert!(EventKind::parse("TaskCreated").is_err());
        assert!(EventKind::parse("").is_err());
    }

    #[test]
    fn kind_serializes_as_its_wire_name() {
        assert_eq!(
            serde_json::to_string(&EventKind::TaskStatusChanged).unwrap(),
            "\"task_status_changed\""
        );
        let parsed: EventKind = serde_json::from_str("\"gate_finished\"").unwrap();
        assert_eq!(parsed, EventKind::GateFinished);
    }

    #[test]
    fn builder_sets_scopes_and_payload() {
        let project = ProjectId::new();
        let task = TaskId::new();
        let event = NewEvent::new(EventKind::TaskStatusChanged)
            .project(&project)
            .task(&task)
            .payload(serde_json::json!({ "from": "running", "to": "verifying" }))
            .into_event(7);

        assert_eq!(event.seq, 7);
        assert_eq!(event.project_id.as_ref(), Some(&project));
        assert_eq!(event.task_id.as_ref(), Some(&task));
        assert_eq!(event.payload["to"], "verifying");
        assert!(event.goal_id.is_none());
    }

    #[test]
    fn timestamps_encode_with_fixed_precision_utc() {
        let ts: Timestamp = "2026-11-21T10:00:00.123456+02:00".parse().unwrap();
        let encoded = Event::encode_ts(ts);
        assert!(encoded.ends_with('Z'), "{encoded}");
        assert_eq!(encoded, "2026-11-21T08:00:00.123Z");
    }

    #[test]
    fn audit_kinds_are_flagged_permanent() {
        assert!(EventKind::CommitCreated.is_audit_permanent());
        assert!(EventKind::RunFinished.is_audit_permanent());
        assert!(!EventKind::AgentText.is_audit_permanent());
    }
}
