use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("cannot parse {path}: {reason}")]
    Parse { path: String, reason: String },

    #[error("embedding provider unavailable: {0}")]
    Embedding(String),

    #[error("invalid request: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, MemoryError>;
