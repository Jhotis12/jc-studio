//! Thin client for the daemon.
//!
//! One connection per request: a CLI makes a handful of calls, and connection pooling
//! would add state for no benefit. The event stream keeps its connection open and parses
//! newline-delimited JSON incrementally.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::client::conn::http1;
use hyper::Request;
use hyper_util::rt::TokioIo;
use jcs_proto::{PROTOCOL_HEADER, PROTOCOL_VERSION};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::net::UnixStream;

#[derive(Debug, Clone)]
pub struct Client {
    socket: PathBuf,
}

impl Client {
    pub fn new(socket: impl Into<PathBuf>) -> Self {
        Self {
            socket: socket.into(),
        }
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    pub async fn health(&self) -> Result<jcs_proto::HealthResponse> {
        self.get("/health").await
    }

    pub async fn status(&self) -> Result<jcs_proto::StatusResponse> {
        self.get("/status").await
    }

    pub async fn projects(&self) -> Result<jcs_proto::ProjectListResponse> {
        self.get("/projects").await
    }

    pub async fn add_project(
        &self,
        request: &jcs_proto::AddProjectRequest,
    ) -> Result<jcs_proto::ProjectResponse> {
        self.post("/projects", request).await
    }

    pub async fn update_project(
        &self,
        id: &str,
        settings: &jcs_proto::ProjectSettings,
    ) -> Result<jcs_proto::ProjectResponse> {
        self.put(&format!("/projects/{id}"), settings).await
    }

    pub async fn remove_project(&self, id: &str) -> Result<jcs_proto::OkResponse> {
        self.delete(&format!("/projects/{id}")).await
    }

    pub async fn ask(&self, request: &jcs_proto::AskRequest) -> Result<jcs_proto::AskResponse> {
        self.post("/ask", request).await
    }

    pub async fn abort(&self, run_id: &str) -> Result<jcs_proto::OkResponse> {
        let body = jcs_proto::AbortRequest {
            run_id: run_id.to_string(),
        };
        self.post("/abort", &body).await
    }

    pub async fn runs(&self) -> Result<Vec<jcs_proto::RunView>> {
        self.get("/runs").await
    }

    pub async fn run_detail(&self, id: &str) -> Result<jcs_proto::RunDetailResponse> {
        self.get(&format!("/runs/{id}")).await
    }

    pub async fn models(&self) -> Result<Vec<jcs_proto::ModelCatalogEntry>> {
        self.get("/catalog/models").await
    }

    pub async fn skills(&self, project: &str) -> Result<Vec<jcs_proto::SkillCatalogEntry>> {
        self.get(&format!("/catalog/skills?project={}", encode(project)))
            .await
    }

    pub async fn mcp_servers(&self, project: &str) -> Result<Vec<jcs_proto::McpServerEntry>> {
        self.get(&format!("/catalog/mcp?project={}", encode(project)))
            .await
    }

    pub async fn mcp_connect(&self, project: &str, server: &str) -> Result<jcs_proto::OkResponse> {
        self.post(
            &format!("/catalog/mcp/{server}/connect?project={}", encode(project)),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn mcp_disconnect(
        &self,
        project: &str,
        server: &str,
    ) -> Result<jcs_proto::OkResponse> {
        self.post(
            &format!(
                "/catalog/mcp/{server}/disconnect?project={}",
                encode(project)
            ),
            &serde_json::json!({}),
        )
        .await
    }

    pub async fn mcp_add(
        &self,
        request: &jcs_proto::McpAddRequest,
    ) -> Result<jcs_proto::OkResponse> {
        self.post("/catalog/mcp", request).await
    }

    pub async fn mcp_remove(&self, project: &str, server: &str) -> Result<jcs_proto::OkResponse> {
        self.delete(&format!(
            "/catalog/mcp/{server}?project={}",
            encode(project)
        ))
        .await
    }

    pub async fn settings(&self) -> Result<jcs_proto::SettingsView> {
        self.get("/settings").await
    }

    pub async fn update_settings(
        &self,
        patch: &jcs_proto::SettingsPatch,
    ) -> Result<jcs_proto::SettingsView> {
        self.put("/settings", patch).await
    }

    pub async fn metrics(&self) -> Result<String> {
        let response = self.send(hyper::Method::GET, "/metrics", None).await?;
        let status = response.status();
        let body = response
            .into_body()
            .collect()
            .await
            .map(|collected| collected.to_bytes())
            .unwrap_or_default();

        if !status.is_success() {
            anyhow::bail!("daemon returned {status} for /metrics");
        }
        Ok(String::from_utf8_lossy(&body).to_string())
    }

    pub async fn memory_index(
        &self,
        project: &str,
        force: bool,
    ) -> Result<jcs_proto::MemoryIndexResponse> {
        let request = jcs_proto::MemoryIndexRequest {
            project: project.to_string(),
            force,
        };
        self.post("/memory/index", &request).await
    }

    pub async fn memory_search(
        &self,
        project: &str,
        query: &str,
        limit: usize,
    ) -> Result<jcs_proto::MemorySearchResponse> {
        self.get(&format!(
            "/memory/search?project={}&q={}&limit={limit}",
            encode(project),
            encode(query)
        ))
        .await
    }

    pub async fn memory_stats(&self, project: &str) -> Result<jcs_proto::MemoryStatsView> {
        self.get(&format!("/memory/stats?project={}", encode(project)))
            .await
    }

    pub async fn goal_detail(&self, id: &str) -> Result<jcs_proto::GoalDetailResponse> {
        self.get(&format!("/goals/{id}")).await
    }

    pub async fn cancel_goal(&self, id: &str) -> Result<jcs_proto::GoalActionResponse> {
        self.post(&format!("/goals/{id}/cancel"), &serde_json::json!({}))
            .await
    }

    pub async fn delete_goal(&self, id: &str) -> Result<jcs_proto::OkResponse> {
        self.delete(&format!("/goals/{id}")).await
    }

    pub async fn resume_goal(&self, id: &str) -> Result<jcs_proto::GoalActionResponse> {
        self.post(&format!("/goals/{id}/resume"), &serde_json::json!({}))
            .await
    }

    pub async fn policy(&self, project: &str) -> Result<jcs_proto::PolicyView> {
        self.get(&format!("/policy?project={}", encode(project)))
            .await
    }

    pub async fn policy_packs(&self) -> Result<Vec<jcs_proto::PackCatalogEntry>> {
        self.get("/policy/packs").await
    }

    pub async fn render_policy(
        &self,
        request: &jcs_proto::RenderPolicyRequest,
    ) -> Result<jcs_proto::RenderPolicyResponse> {
        self.post("/policy/render", request).await
    }

    pub async fn gates(&self, project: &str) -> Result<Vec<jcs_proto::PolicyGateView>> {
        self.get(&format!("/gates?project={}", encode(project)))
            .await
    }

    pub async fn run_gates(
        &self,
        request: &jcs_proto::GateRunRequest,
    ) -> Result<jcs_proto::GateRunResponse> {
        self.post("/gates/run", request).await
    }

    /// Bounded event history for one task, oldest first.
    pub async fn task_events(&self, task: &str, limit: i64) -> Result<Vec<jcs_core::Event>> {
        self.get(&format!("/tasks/{task}/events?limit={limit}"))
            .await
    }

    /// Bounded event history. With `since = None` it returns the most recent events.
    pub async fn event_history(
        &self,
        since: Option<i64>,
        limit: i64,
    ) -> Result<Vec<jcs_core::Event>> {
        let mut path = format!("/events/history?limit={limit}");
        if let Some(since) = since {
            path.push_str(&format!("&since={since}"));
        }
        self.get(&path).await
    }

    /// Current end of the log, used as a cursor before starting a run.
    pub async fn latest_seq(&self) -> Result<i64> {
        let events = self.event_history(None, 1).await?;
        Ok(events.last().map(|event| event.seq).unwrap_or(0))
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let response = self.send(hyper::Method::GET, path, None).await?;
        decode(path, response).await
    }

    pub async fn post<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        let bytes = serde_json::to_vec(body).context("cannot serialize the request")?;
        let response = self
            .send(hyper::Method::POST, path, Some(Bytes::from(bytes)))
            .await?;
        decode(path, response).await
    }

    pub async fn put<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        let bytes = serde_json::to_vec(body).context("cannot serialize the request")?;
        let response = self
            .send(hyper::Method::PUT, path, Some(Bytes::from(bytes)))
            .await?;
        decode(path, response).await
    }

    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let response = self.send(hyper::Method::DELETE, path, None).await?;
        decode(path, response).await
    }

    /// Opens the event stream starting strictly after `since`.
    pub async fn events(&self, since: i64) -> Result<EventStream> {
        let response = self
            .send(hyper::Method::GET, &format!("/events?since={since}"), None)
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .into_body()
                .collect()
                .await
                .map(|collected| String::from_utf8_lossy(&collected.to_bytes()).to_string())
                .unwrap_or_default();
            bail!("daemon returned {status} for /events: {body}");
        }

        Ok(EventStream {
            body: response.into_body(),
            buffer: Vec::new(),
            done: false,
        })
    }

    async fn send(
        &self,
        method: hyper::Method,
        path: &str,
        body: Option<Bytes>,
    ) -> Result<hyper::Response<Incoming>> {
        let stream = UnixStream::connect(&self.socket).await.with_context(|| {
            format!(
                "cannot reach the daemon at {} (start it with `jcsd`, or run `jcs doctor`)",
                self.socket.display()
            )
        })?;

        let io = TokioIo::new(stream);
        let (mut sender, connection) = http1::handshake(io)
            .await
            .context("cannot establish an HTTP session with the daemon")?;

        // The connection future must be driven for the response body to arrive.
        tokio::spawn(async move {
            let _ = connection.await;
        });

        let payload = body.unwrap_or_default();
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header(PROTOCOL_HEADER, PROTOCOL_VERSION.to_string())
            .header(hyper::header::CONTENT_TYPE, "application/json")
            .body(Full::new(payload))
            .context("cannot build the request")?;

        sender
            .send_request(request)
            .await
            .context("the daemon closed the connection")
    }
}

/// Percent-encodes a query value without pulling in a URL crate.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

async fn decode<T: DeserializeOwned>(path: &str, response: hyper::Response<Incoming>) -> Result<T> {
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .map(|collected| collected.to_bytes())
        .unwrap_or_default();

    if !status.is_success() {
        let text = String::from_utf8_lossy(&body);
        let message = serde_json::from_str::<jcs_proto::ErrorBody>(&text)
            .map(|error| format!("{}: {}", error.code, error.message))
            .unwrap_or_else(|_| text.trim().to_string());
        bail!("{message}");
    }

    serde_json::from_slice(&body)
        .map_err(|error| anyhow!("cannot parse the daemon response for {path}: {error}"))
}

/// Incremental reader for the daemon's NDJSON event stream.
pub struct EventStream {
    body: Incoming,
    buffer: Vec<u8>,
    done: bool,
}

impl EventStream {
    /// Returns the next event, or `None` when the stream closes.
    pub async fn next(&mut self) -> Option<Result<jcs_core::Event>> {
        loop {
            if let Some(index) = self.buffer.iter().position(|byte| *byte == b'\n') {
                let line: Vec<u8> = self.buffer.drain(..=index).collect();
                let text =
                    String::from_utf8_lossy(&line[..line.len().saturating_sub(1)]).to_string();
                if text.trim().is_empty() {
                    continue;
                }
                return Some(
                    serde_json::from_str::<jcs_core::Event>(&text)
                        .map_err(|error| anyhow!("cannot parse an event: {error}")),
                );
            }

            if self.done {
                return None;
            }

            match self.body.frame().await {
                None => {
                    self.done = true;
                }
                Some(Err(error)) => {
                    self.done = true;
                    return Some(Err(anyhow!("event stream error: {error}")));
                }
                Some(Ok(frame)) => {
                    if let Ok(data) = frame.into_data() {
                        self.buffer.extend_from_slice(&data);
                    }
                }
            }
        }
    }
}
