use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("no opencode service found: {0}")]
    Discovery(String),

    #[error("cannot reach opencode at {url}: {source}")]
    Transport {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("opencode rejected the request ({status}) for {operation}: {message}")]
    Api {
        status: u16,
        operation: String,
        message: String,
    },

    #[error("opencode returned an unexpected body for {operation}: {message}")]
    Decode { operation: String, message: String },

    #[error("stream error: {0}")]
    Stream(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AgentError {
    /// Whether retrying after a short delay could plausibly succeed.
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Transport { .. } | Self::Stream(_) => true,
            Self::Api { status, .. } => *status == 429 || *status >= 500,
            _ => false,
        }
    }

    /// True when the credential was rejected, as opposed to a missing service.
    pub fn is_auth_failure(&self) -> bool {
        matches!(self, Self::Api { status: 401 | 403, .. })
    }
}

impl AgentError {
    pub(crate) fn decode(operation: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Decode {
            operation: operation.into(),
            message: message.into(),
        }
    }

    pub(crate) fn discovery(message: impl Into<String>) -> Self {
        Self::Discovery(message.into())
    }

    pub(crate) fn stream(message: impl Into<String>) -> Self {
        Self::Stream(message.into())
    }
}

/// Location of the local OpenCode configuration directory.
pub(crate) fn opencode_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("opencode"))
}

pub type Result<T> = std::result::Result<T, AgentError>;
