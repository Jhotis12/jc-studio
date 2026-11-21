//! Agent backend client for JC Studio.
//!
//! The backend is OpenCode v2 over its local HTTP API. Discovery is explicit
//! (`JCS_OPENCODE_URL`/`JCS_OPENCODE_PASSWORD`) or automatic through the OpenCode CLI
//! and its credential file; credentials are redacted from `Debug` and never logged.
//!
//! The SSE framing here is hand-written because the OpenCode stream carries the event
//! type inside the JSON payload rather than in the `event:` field (ADR 0002).

pub mod client;
pub mod discovery;
pub mod error;
pub mod events;
pub mod sse;

pub use client::{CreateSession, EventStream, ModelRef, OpenCodeClient, ServerInfo};
pub use discovery::Endpoint;
pub use error::{AgentError, Result};
pub use events::{AgentEvent, ExecutionOutcome, ToolInvocation, Usage};
pub use sse::{SseDecoder, SseFrame};
