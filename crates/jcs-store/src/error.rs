use std::path::PathBuf;

use thiserror::Error;

/// Failures of the persistence layer.
///
/// Domain validation errors are preserved as a distinct variant so callers can tell
/// "you asked for an illegal transition" (a bug in orchestration) from "the disk is
/// full" (an infrastructure problem).
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("domain error: {0}")]
    Domain(#[from] jcs_core::CoreError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("another instance already owns {path}")]
    Locked { path: PathBuf },

    #[error("not found: {entity} {id}")]
    NotFound { entity: &'static str, id: String },

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("corrupt row in {table}: {detail}")]
    Corrupt { table: &'static str, detail: String },

    #[error("payload serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

impl StoreError {
    pub(crate) fn not_found(entity: &'static str, id: impl ToString) -> Self {
        Self::NotFound {
            entity,
            id: id.to_string(),
        }
    }

    /// Whether retrying the operation later could plausibly succeed.
    pub fn is_transient(&self) -> bool {
        match self {
            Self::Db(sqlx::Error::PoolTimedOut) | Self::Db(sqlx::Error::PoolClosed) => true,
            Self::Db(sqlx::Error::Database(db)) => {
                let msg = db.message().to_ascii_lowercase();
                msg.contains("database is locked") || msg.contains("busy")
            }
            _ => false,
        }
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;
