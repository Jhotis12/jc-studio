use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::discovery::Endpoint;
use crate::error::{AgentError, Result};
use crate::events::AgentEvent;
use crate::sse::SseDecoder;

/// Timeout for ordinary request/response calls. Streaming is exempt.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Reference to a model, in the shape the API expects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    pub id: String,
    #[serde(rename = "providerID")]
    pub provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

impl ModelRef {
    /// Parses `provider/model`, the form used on the command line.
    pub fn parse(reference: &str) -> Result<Self> {
        let (provider, id) = reference.split_once('/').ok_or_else(|| {
            AgentError::decode(
                "model",
                format!("expected `provider/model`, got {reference:?}"),
            )
        })?;
        if provider.is_empty() || id.is_empty() {
            return Err(AgentError::decode(
                "model",
                format!("expected `provider/model`, got {reference:?}"),
            ));
        }
        Ok(Self {
            id: id.to_string(),
            provider_id: provider.to_string(),
            variant: None,
        })
    }

    pub fn to_reference(&self) -> String {
        format!("{}/{}", self.provider_id, self.id)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerInfo {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub pid: Option<i64>,
    #[serde(default)]
    pub urls: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentSession {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionState {
    pub id: String,
    /// The backend's own verdict for the last completed turn. It persists across prompts,
    /// so it is only meaningful together with `time.is_idle()`.
    #[serde(default)]
    pub outcome: Option<String>,
    #[serde(default)]
    pub cost: Option<f64>,
    #[serde(default)]
    pub time: SessionTime,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct SessionTime {
    /// Unix milliseconds when the current turn started.
    #[serde(default)]
    pub busy: Option<i64>,
    /// Unix milliseconds when the last turn finished.
    #[serde(default)]
    pub idle: Option<i64>,
}

impl SessionTime {
    /// Whether the session is between turns *right now*.
    ///
    /// `outcome` is not cleared when a new prompt arrives (verified against 2.3.2), so the
    /// only reliable way to tell "finished" from "finished last time" is comparing these
    /// two timestamps. A session that never ran has no idle time and is not idle.
    pub fn is_idle(&self) -> bool {
        match (self.idle, self.busy) {
            (Some(idle), Some(busy)) => idle > busy,
            (Some(_), None) => true,
            (None, _) => false,
        }
    }
}

/// Input for creating a session bound to a directory.
#[derive(Debug, Clone)]
pub struct CreateSession<'a> {
    pub title: &'a str,
    /// Absolute directory the session is confined to (a task worktree).
    pub directory: &'a Path,
    pub model: Option<ModelRef>,
    pub agent: Option<&'a str>,
    /// Permission ruleset in the backend's format, when policy rendering is active.
    pub permissions: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PromptAck {
    #[serde(default)]
    pub id: Option<String>,
}

/// HTTP client for a local OpenCode v2 service.
#[derive(Debug, Clone)]
pub struct OpenCodeClient {
    http: reqwest::Client,
    endpoint: Endpoint,
}

impl OpenCodeClient {
    pub fn new(endpoint: Endpoint) -> Result<Self> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| AgentError::decode("http client", e.to_string()))?;
        Ok(Self { http, endpoint })
    }

    /// Discovers the local service and builds a client for it.
    pub fn discover() -> Result<Self> {
        Self::new(Endpoint::discover()?)
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.endpoint.base_url(), path)
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
        timeout: Option<Duration>,
    ) -> Result<Value> {
        let mut request = self
            .http
            .request(method, self.url(path))
            .basic_auth("opencode", Some(self.endpoint.password()));

        if let Some(limit) = timeout {
            request = request.timeout(limit);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request
            .send()
            .await
            .map_err(|source| AgentError::Transport {
                url: self.endpoint.base_url().to_string(),
                source,
            })?;

        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|source| AgentError::Transport {
                url: self.endpoint.base_url().to_string(),
                source,
            })?;

        if !status.is_success() {
            return Err(AgentError::Api {
                status: status.as_u16(),
                operation: path.to_string(),
                message: extract_error_message(&text),
            });
        }

        if text.trim().is_empty() {
            return Ok(Value::Null);
        }

        serde_json::from_str(&text)
            .map_err(|e| AgentError::decode(path, format!("{e}: {}", truncate(&text, 300))))
    }

    /// Liveness plus version detection.
    pub async fn health(&self) -> Result<ServerInfo> {
        let value = self
            .request(
                reqwest::Method::GET,
                "/api/info",
                None,
                Some(REQUEST_TIMEOUT),
            )
            .await?;
        serde_json::from_value(value).map_err(|e| AgentError::decode("/api/info", e.to_string()))
    }

    /// Projects the backend already knows about.
    pub async fn projects(&self) -> Result<Vec<Value>> {
        let value = self
            .request(
                reqwest::Method::GET,
                "/api/project",
                None,
                Some(REQUEST_TIMEOUT),
            )
            .await?;
        Ok(into_list(value))
    }

    /// Models available to this instance.
    pub async fn models(&self) -> Result<Vec<ModelRef>> {
        let value = self
            .request(
                reqwest::Method::GET,
                "/api/model",
                None,
                Some(REQUEST_TIMEOUT),
            )
            .await?;
        let mut models = Vec::new();
        for item in into_list(value) {
            let id = item.get("id").and_then(Value::as_str).unwrap_or_default();
            let provider = item
                .get("providerID")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !id.is_empty() && !provider.is_empty() {
                models.push(ModelRef {
                    id: id.to_string(),
                    provider_id: provider.to_string(),
                    variant: None,
                });
            }
        }
        Ok(models)
    }

    /// Creates a session confined to `directory`.
    pub async fn create_session(&self, input: CreateSession<'_>) -> Result<AgentSession> {
        let mut body = json!({
            "title": input.title,
            "location": { "directory": input.directory.to_string_lossy() },
        });

        if let Some(model) = input.model {
            body["model"] = serde_json::to_value(&model)?;
        }
        if let Some(agent) = input.agent {
            body["agent"] = json!(agent);
        }
        if let Some(permissions) = input.permissions {
            body["permissions"] = permissions;
        }

        let value = self
            .request(
                reqwest::Method::POST,
                "/api/session",
                Some(body),
                Some(REQUEST_TIMEOUT),
            )
            .await?;
        let data = unwrap_data(value);
        serde_json::from_value(data).map_err(|e| AgentError::decode("/api/session", e.to_string()))
    }

    /// Sends a prompt to a session.
    ///
    /// `steer` delivers into the current turn when one is running; `queue` waits for it
    /// to finish. Orchestration uses `queue` so a repair prompt cannot interleave with
    /// the work it is repairing.
    pub async fn prompt(&self, session_id: &str, text: &str, delivery: &str) -> Result<PromptAck> {
        let body = json!({ "text": text, "delivery": delivery });
        let value = self
            .request(
                reqwest::Method::POST,
                &format!("/api/session/{session_id}/prompt"),
                Some(body),
                Some(REQUEST_TIMEOUT),
            )
            .await?;
        let data = unwrap_data(value);
        Ok(serde_json::from_value(data).unwrap_or(PromptAck { id: None }))
    }

    pub async fn interrupt(&self, session_id: &str) -> Result<()> {
        self.request(
            reqwest::Method::POST,
            &format!("/api/session/{session_id}/interrupt"),
            None,
            Some(REQUEST_TIMEOUT),
        )
        .await?;
        Ok(())
    }

    /// Session metadata, including the backend's own view of the outcome.
    pub async fn session(&self, session_id: &str) -> Result<SessionState> {
        let value = self
            .request(
                reqwest::Method::GET,
                &format!("/api/session/{session_id}"),
                None,
                Some(REQUEST_TIMEOUT),
            )
            .await?;
        let data = unwrap_data(value);
        serde_json::from_value(data)
            .map_err(|e| AgentError::decode("/api/session/{id}", e.to_string()))
            .and_then(|state: SessionState| {
                if state.id.is_empty() {
                    Err(AgentError::decode(
                        "/api/session/{id}",
                        "response has no id",
                    ))
                } else {
                    Ok(state)
                }
            })
    }

    /// Message history, newest first as the backend returns it.
    pub async fn messages(&self, session_id: &str, limit: u32) -> Result<Vec<Value>> {
        let value = self
            .request(
                reqwest::Method::GET,
                &format!("/api/session/{session_id}/message?limit={limit}"),
                None,
                Some(REQUEST_TIMEOUT),
            )
            .await?;
        Ok(into_list(unwrap_data(value)))
    }

    /// Unified diff produced by the session, as raw entries.
    ///
    /// Kept untyped on purpose: the shape is richer than the fields we currently use,
    /// and a typed struct would discard information the TUI may want later.
    pub async fn diff(&self, session_id: &str) -> Result<Vec<Value>> {
        let value = self
            .request(
                reqwest::Method::GET,
                &format!("/api/session/{session_id}/diff"),
                None,
                Some(REQUEST_TIMEOUT),
            )
            .await?;
        Ok(into_list(unwrap_data(value)))
    }

    /// Permissions the backend is currently waiting on.
    pub async fn pending_permissions(&self, session_id: &str) -> Result<Vec<Value>> {
        let value = self
            .request(
                reqwest::Method::GET,
                &format!("/api/session/{session_id}/permission"),
                None,
                Some(REQUEST_TIMEOUT),
            )
            .await?;
        Ok(into_list(unwrap_data(value)))
    }

    /// Interactive forms (questions, structured prompts) the session is waiting on.
    ///
    /// A batch orchestrator must not hang on these: they are surfaced as a blocked task
    /// and cancelled, so the backend is not left holding a turn open.
    pub async fn list_forms(&self, session_id: &str) -> Result<Vec<Value>> {
        let value = self
            .request(
                reqwest::Method::GET,
                &format!("/api/session/{session_id}/form"),
                None,
                Some(REQUEST_TIMEOUT),
            )
            .await?;
        Ok(into_list(unwrap_data(value)))
    }

    pub async fn cancel_form(&self, session_id: &str, form_id: &str) -> Result<()> {
        self.request(
            reqwest::Method::DELETE,
            &format!("/api/session/{session_id}/form/{form_id}"),
            None,
            Some(REQUEST_TIMEOUT),
        )
        .await?;
        Ok(())
    }

    /// Answers a permission request: `once`, `always` or `reject`.
    pub async fn reply_permission(
        &self,
        session_id: &str,
        request_id: &str,
        decision: &str,
        message: Option<&str>,
    ) -> Result<()> {
        let mut body = json!({ "decision": decision });
        if let Some(message) = message {
            body["message"] = json!(message);
        }
        self.request(
            reqwest::Method::POST,
            &format!("/api/session/{session_id}/permission/{request_id}/reply"),
            Some(body),
            Some(REQUEST_TIMEOUT),
        )
        .await?;
        Ok(())
    }

    /// Opens the live event stream (no timeout: it is long-lived).
    pub async fn events(&self) -> Result<EventStream> {
        let response = self
            .http
            .get(self.url("/api/event"))
            .basic_auth("opencode", Some(self.endpoint.password()))
            .send()
            .await
            .map_err(|source| AgentError::Transport {
                url: self.endpoint.base_url().to_string(),
                source,
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(AgentError::Api {
                status,
                operation: "/api/event".to_string(),
                message: extract_error_message(&body),
            });
        }

        Ok(EventStream {
            stream: response.bytes_stream().boxed(),
            decoder: SseDecoder::new(),
            pending: std::collections::VecDeque::new(),
            done: false,
        })
    }
}

/// A long-lived stream of decoded backend events.
pub struct EventStream {
    stream: Pin<Box<dyn Stream<Item = reqwest::Result<Bytes>> + Send>>,
    decoder: SseDecoder,
    pending: std::collections::VecDeque<AgentEvent>,
    done: bool,
}

impl EventStream {
    /// Returns the next decoded event, or `None` when the stream ends.
    ///
    /// Heartbeat comments do not produce events; they only keep the connection alive.
    pub async fn next_event(&mut self) -> Option<Result<AgentEvent>> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Some(Ok(event));
            }
            if self.done {
                return None;
            }

            match self.stream.next().await {
                None => {
                    self.done = true;
                    if let Some(frame) = self.decoder.finish() {
                        if !frame.is_empty() {
                            return Some(AgentEvent::from_frame(frame));
                        }
                    }
                    return None;
                }
                Some(Err(error)) => {
                    self.done = true;
                    return Some(Err(AgentError::stream(error.to_string())));
                }
                Some(Ok(bytes)) => {
                    for frame in self.decoder.feed(&bytes) {
                        if frame.is_empty() {
                            continue;
                        }
                        match AgentEvent::from_frame(frame) {
                            Ok(event) => self.pending.push_back(event),
                            Err(error) => return Some(Err(error)),
                        }
                    }
                }
            }
        }
    }
}

/// Many endpoints wrap their payload in `{"data": ...}`; `project.list` does not.
fn unwrap_data(value: Value) -> Value {
    match value {
        Value::Object(ref map) if map.len() == 1 && map.contains_key("data") => {
            map.get("data").cloned().unwrap_or(Value::Null)
        }
        other => other,
    }
}

fn into_list(value: Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items,
        Value::Object(map) => match map.get("data") {
            Some(Value::Array(items)) => items.clone(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn extract_error_message(body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        if let Some(message) = value.get("message").and_then(Value::as_str) {
            return message.to_string();
        }
        if let Some(message) = value.get("error").and_then(Value::as_str) {
            return message.to_string();
        }
    }
    truncate(body.trim(), 300)
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

    #[test]
    fn model_reference_parses_and_roundtrips() {
        let model = ModelRef::parse("opencode/hy4-flash-free").unwrap();
        assert_eq!(model.provider_id, "opencode");
        assert_eq!(model.id, "hy4-flash-free");
        assert_eq!(model.to_reference(), "opencode/hy4-flash-free");
    }

    #[test]
    fn model_reference_rejects_malformed_input() {
        assert!(ModelRef::parse("no-slash").is_err());
        assert!(ModelRef::parse("/model").is_err());
        assert!(ModelRef::parse("provider/").is_err());
    }

    #[test]
    fn data_envelope_is_unwrapped_but_bare_payloads_survive() {
        assert_eq!(unwrap_data(json!({"data": 1})), json!(1));
        assert_eq!(unwrap_data(json!([1, 2])), json!([1, 2]));
        // An entity that legitimately has a `data` field must not be unwrapped.
        let entity = json!({"data": 1, "id": "x"});
        assert_eq!(unwrap_data(entity.clone()), entity);
    }

    #[test]
    fn lists_are_extracted_from_both_shapes() {
        assert_eq!(into_list(json!([1, 2])).len(), 2);
        assert_eq!(into_list(json!({"data": [1, 2, 3]})).len(), 3);
        assert!(into_list(json!(null)).is_empty());
    }

    #[test]
    fn api_errors_surface_the_server_message() {
        let message = extract_error_message(
            r#"{"_tag":"UnauthorizedError","message":"Authentication required"}"#,
        );
        assert_eq!(message, "Authentication required");
    }

    #[test]
    fn a_session_is_idle_only_after_the_current_turn_finished() {
        // Never ran.
        assert!(!SessionTime::default().is_idle());

        // Running: busy is newer than the previous idle mark.
        let time = SessionTime {
            idle: Some(1_000),
            busy: Some(2_000),
        };
        assert!(!time.is_idle());

        // Finished: idle came after busy.
        let time = SessionTime {
            busy: Some(2_000),
            idle: Some(2_100),
        };
        assert!(time.is_idle());
    }

    #[test]
    fn auth_failures_are_classified_separately_from_transport_failures() {
        let auth = AgentError::Api {
            status: 401,
            operation: "/api/info".into(),
            message: "Authentication required".into(),
        };
        assert!(auth.is_auth_failure());
        assert!(!auth.is_transient());

        let busy = AgentError::Api {
            status: 503,
            operation: "/api/info".into(),
            message: "unavailable".into(),
        };
        assert!(busy.is_transient());
        assert!(!busy.is_auth_failure());
    }
}
