use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use jcs_agent::AgentError;
use jcs_git::GitError;
use jcs_proto::ErrorBody;
use jcs_store::StoreError;

/// An HTTP error with a stable machine code.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: sanitize(message.into()),
        }
    }

    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub fn not_found(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message)
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    pub fn unavailable(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, code, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(ErrorBody::new(self.code, self.message))).into_response()
    }
}

impl From<StoreError> for ApiError {
    fn from(error: StoreError) -> Self {
        match &error {
            StoreError::NotFound { .. } => Self::not_found("not_found", error.to_string()),
            StoreError::Conflict(_) => Self::conflict("conflict", error.to_string()),
            StoreError::Domain(_) => Self::bad_request("invalid_transition", error.to_string()),
            StoreError::Locked { .. } => Self::conflict("database_locked", error.to_string()),
            StoreError::Db(_) | StoreError::Migrate(_) | StoreError::Io(_) => {
                tracing::error!(error = %error, "storage failure");
                Self::internal(error.to_string())
            }
            StoreError::Corrupt { .. } | StoreError::Json(_) => {
                tracing::error!(error = %error, "corrupt state");
                Self::internal(error.to_string())
            }
        }
    }
}

impl From<AgentError> for ApiError {
    fn from(error: AgentError) -> Self {
        match &error {
            AgentError::Discovery(_) | AgentError::Transport { .. } => {
                Self::unavailable("agent_unavailable", error.to_string())
            }
            AgentError::Api { status, .. } if *status == 401 || *status == 403 => Self::new(
                StatusCode::BAD_GATEWAY,
                "agent_auth_failed",
                error.to_string(),
            ),
            _ => Self::new(StatusCode::BAD_GATEWAY, "agent_error", error.to_string()),
        }
    }
}

impl From<GitError> for ApiError {
    fn from(error: GitError) -> Self {
        match &error {
            GitError::NotARepository { .. } => {
                Self::bad_request("not_a_repository", error.to_string())
            }
            GitError::InvalidMessage(_) => {
                Self::bad_request("invalid_commit_message", error.to_string())
            }
            GitError::Refused(_) => Self::conflict("git_refused", error.to_string()),
            GitError::Command { .. } => {
                tracing::error!(error = %error, "git failure");
                Self::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "git_failed",
                    error.to_string(),
                )
            }
            GitError::Io(_) => Self::internal(error.to_string()),
        }
    }
}

impl From<jcs_core::CoreError> for ApiError {
    fn from(error: jcs_core::CoreError) -> Self {
        Self::bad_request("invalid_request", error.to_string())
    }
}

/// Collapses multi-line internal messages so the JSON body stays readable.
fn sanitize(message: String) -> String {
    let collapsed = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > 800 {
        collapsed.chars().take(800).collect::<String>() + "…"
    } else {
        collapsed
    }
}

pub type ApiResult<T> = std::result::Result<T, ApiError>;
