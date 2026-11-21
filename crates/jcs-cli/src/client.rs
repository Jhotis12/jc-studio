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
