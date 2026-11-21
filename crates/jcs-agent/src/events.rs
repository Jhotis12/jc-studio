//! Typed views over the OpenCode event stream.
//!
//! Ground truth for these shapes comes from a captured session against OpenCode
//! 2.3.2 (see `docs/contracts/CONTRACT.md`). Unknown event kinds are preserved as
//! strings with their raw payload instead of failing, so a minor server upgrade
//! degrades observability rather than breaking the daemon.

use serde_json::Value;

use crate::error::{AgentError, Result};
use crate::sse::SseFrame;

/// Cumulative usage reported by the backend for a session.
///
/// The backend reports running totals, so this replaces rather than accumulates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Usage {
    pub cost_usd: f64,
    pub tokens_in: i64,
    pub tokens_out: i64,
}

/// How an execution ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionOutcome {
    Succeeded,
    Failed,
    Interrupted,
}

/// A tool invocation, as far as the event stream exposes it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolInvocation {
    pub name: String,
    pub call_id: Option<String>,
    pub input: Option<Value>,
    pub output: Option<Value>,
    pub failed: bool,
}

/// One decoded backend event.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentEvent {
    pub id: String,
    /// Server timestamp in Unix milliseconds, when present.
    pub created: Option<i64>,
    /// Dot-separated event kind, e.g. `session.execution.succeeded`.
    pub kind: String,
    pub data: Value,
}

impl AgentEvent {
    /// Decodes the JSON body of an SSE frame.
    ///
    /// The OpenCode stream carries the event type inside the payload, not in an
    /// `event:` field, which is why this does not look at `frame.event`.
    pub fn from_frame(frame: SseFrame) -> Result<Self> {
        let payload: Value = serde_json::from_str(&frame.data).map_err(|e| {
            AgentError::decode(
                "agent event",
                format!(
                    "event payload is not JSON ({e}): {}",
                    truncate(&frame.data, 200)
                ),
            )
        })?;

        let id = payload
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let created = payload.get("created").and_then(Value::as_i64);
        let kind = payload
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let data = payload.get("data").cloned().unwrap_or(Value::Null);

        Ok(Self {
            id,
            created,
            kind,
            data,
        })
    }

    /// Session this event belongs to, when it is session-scoped.
    pub fn session_id(&self) -> Option<&str> {
        self.data.get("sessionID").and_then(Value::as_str)
    }

    /// Incremental assistant text, for live rendering.
    pub fn text_delta(&self) -> Option<&str> {
        if self.kind != "session.text.delta" {
            return None;
        }
        self.data.get("delta").and_then(Value::as_str)
    }

    /// Incremental reasoning text, when the provider exposes it.
    pub fn reasoning_delta(&self) -> Option<&str> {
        if self.kind != "session.reasoning.delta" {
            return None;
        }
        self.data.get("delta").and_then(Value::as_str)
    }

    /// Cumulative usage, if this event reports any.
    pub fn usage(&self) -> Option<Usage> {
        let cost = self.data.get("cost").and_then(Value::as_f64);
        let tokens = self.data.get("tokens");
        if cost.is_none() && tokens.is_none() {
            return None;
        }
        let tokens = tokens?;
        Some(Usage {
            cost_usd: cost.unwrap_or(0.0),
            tokens_in: tokens.get("input").and_then(Value::as_i64).unwrap_or(0),
            tokens_out: tokens.get("output").and_then(Value::as_i64).unwrap_or(0),
        })
    }

    /// Terminal execution outcome, if this event is one.
    pub fn execution_outcome(&self) -> Option<ExecutionOutcome> {
        match self.kind.as_str() {
            "session.execution.succeeded" => Some(ExecutionOutcome::Succeeded),
            "session.execution.failed" => Some(ExecutionOutcome::Failed),
            "session.execution.interrupted" => Some(ExecutionOutcome::Interrupted),
            _ => None,
        }
    }

    /// Tool activity, if this event reports any.
    ///
    /// Field names are read defensively because tool events were not exercised by the
    /// captured fixture: `tool`/`name` and `input`/`args` are both accepted.
    pub fn tool_invocation(&self) -> Option<ToolInvocation> {
        let failed = match self.kind.as_str() {
            "session.tool.called" | "session.tool.progress" => false,
            "session.tool.failed" => true,
            _ => return None,
        };

        let name = self
            .data
            .get("tool")
            .or_else(|| self.data.get("name"))
            .and_then(Value::as_str)?
            .to_string();

        let call_id = self
            .data
            .get("callID")
            .or_else(|| self.data.get("callId"))
            .or_else(|| self.data.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string);

        let input = self
            .data
            .get("input")
            .or_else(|| self.data.get("args"))
            .cloned();
        let output = self
            .data
            .get("output")
            .or_else(|| self.data.get("result"))
            .cloned();

        Some(ToolInvocation {
            name,
            call_id,
            input,
            output,
            failed,
        })
    }

    /// Whether this event ends the current execution for `session_id`.
    pub fn ends_execution_for(&self, session_id: &str) -> Option<ExecutionOutcome> {
        self.execution_outcome()
            .filter(|_| self.session_id() == Some(session_id))
    }

    /// Idle notifications are a secondary completion signal.
    pub fn is_idle_for(&self, session_id: &str) -> bool {
        self.kind == "session.idle" && self.session_id() == Some(session_id)
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    value.chars().take(max).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(payload: &str) -> SseFrame {
        SseFrame {
            event: None,
            id: None,
            data: payload.to_string(),
            retry: None,
        }
    }

    #[test]
    fn decodes_the_real_captured_execution_event() {
        let event = AgentEvent::from_frame(frame(
            r#"{"id":"evt_1","created":1795274772700,"type":"session.execution.succeeded","data":{"sessionID":"ses_abc"}}"#,
        ))
        .unwrap();

        assert_eq!(event.kind, "session.execution.succeeded");
        assert_eq!(event.session_id(), Some("ses_abc"));
        assert_eq!(event.execution_outcome(), Some(ExecutionOutcome::Succeeded));
        assert!(event.ends_execution_for("ses_abc").is_some());
        assert!(event.ends_execution_for("ses_other").is_none());
    }

    #[test]
    fn decodes_the_real_captured_usage_event() {
        let event = AgentEvent::from_frame(frame(
            r#"{"type":"session.usage.updated","data":{"sessionID":"ses_abc","cost":0.0125,"tokens":{"input":40519,"output":3,"reasoning":20,"cache":{"read":2304,"write":0}}}}"#,
        ))
        .unwrap();

        let usage = event.usage().unwrap();
        assert!((usage.cost_usd - 0.0125).abs() < f64::EPSILON);
        assert_eq!(usage.tokens_in, 40519);
        assert_eq!(usage.tokens_out, 3);
    }

    #[test]
    fn extracts_reasoning_deltas_separately_from_text() {
        let event = AgentEvent::from_frame(frame(
            r#"{"type":"session.reasoning.delta","data":{"sessionID":"ses_abc","delta":"checking the schema"}}"#,
        ))
        .unwrap();
        assert_eq!(event.reasoning_delta(), Some("checking the schema"));
        assert_eq!(event.text_delta(), None);
    }

    #[test]
    fn extracts_text_deltas_only_from_delta_events() {
        let delta = AgentEvent::from_frame(frame(
            r#"{"type":"session.text.delta","data":{"sessionID":"ses_abc","delta":"OK"}}"#,
        ))
        .unwrap();
        assert_eq!(delta.text_delta(), Some("OK"));

        let ended = AgentEvent::from_frame(frame(
            r#"{"type":"session.text.ended","data":{"sessionID":"ses_abc","text":"OK"}}"#,
        ))
        .unwrap();
        assert_eq!(ended.text_delta(), None);
    }

    #[test]
    fn unknown_event_kinds_are_preserved_not_rejected() {
        let event = AgentEvent::from_frame(frame(
            r#"{"type":"something.brand.new","data":{"sessionID":"ses_abc","x":1}}"#,
        ))
        .unwrap();
        assert_eq!(event.kind, "something.brand.new");
        assert_eq!(event.data["x"], 1);
        assert!(event.execution_outcome().is_none());
    }

    #[test]
    fn non_json_payloads_are_an_error_with_context() {
        let error = AgentEvent::from_frame(frame("not json")).unwrap_err();
        assert!(error.to_string().contains("not JSON"), "{error}");
    }

    #[test]
    fn tool_events_are_read_defensively() {
        let called = AgentEvent::from_frame(frame(
            r#"{"type":"session.tool.called","data":{"sessionID":"ses_abc","tool":"read","input":{"file":"a.rs"}}}"#,
        ))
        .unwrap();
        let tool = called.tool_invocation().unwrap();
        assert_eq!(tool.name, "read");
        assert_eq!(tool.input.as_ref().unwrap()["file"], "a.rs");
        assert!(!tool.failed);

        let failed = AgentEvent::from_frame(frame(
            r#"{"type":"session.tool.failed","data":{"sessionID":"ses_abc","tool":"bash","output":"exit 1"}}"#,
        ))
        .unwrap();
        assert!(failed.tool_invocation().unwrap().failed);
    }
}
