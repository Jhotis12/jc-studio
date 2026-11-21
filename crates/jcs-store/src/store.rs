use std::fs::File;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use jiff::Timestamp;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{ConnectOptions, SqliteConnection, SqlitePool};
use tracing::debug;

use crate::error::{Result, StoreError};
use jcs_core::Event;

/// Handle to the durable state.
///
/// There is exactly one per daemon: holding it owns the exclusive lock file, and a
/// second `Store::open` on the same database fails instead of silently racing.
pub struct Store {
    pool: SqlitePool,
    /// Kept alive for the lifetime of the store; dropping it releases the OS lock.
    _lock: Option<File>,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Store { .. }")
    }
}

impl Store {
    /// Opens (creating if needed) the database at `path` and applies migrations.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let lock_path = path.with_extension("lock");
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)?;
        if lock.try_lock().is_err() {
            return Err(StoreError::Locked { path: lock_path });
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5))
            .log_statements(tracing::log::LevelFilter::Trace);

        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options)
            .await?;

        let store = Self {
            pool,
            _lock: Some(lock),
        };
        store.migrate().await?;
        debug!(path = %path.display(), "store opened");
        Ok(store)
    }

    /// Opens a throwaway in-memory database. For tests and fixtures only.
    pub async fn in_memory() -> Result<Self> {
        // A single connection keeps the whole database alive for the pool's lifetime;
        // with more connections each one would get its own empty `:memory:` database.
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        let store = Self { pool, _lock: None };
        store.migrate().await?;
        Ok(store)
    }

    /// Runs embedded migrations. Idempotent: sqlx records applied versions.
    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Returns `ok` when the database is structurally sound.
    pub async fn integrity_check(&self) -> Result<String> {
        let row: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&self.pool)
            .await?;
        Ok(row)
    }

    /// Flushes WAL pages into the main database file. Safe to call while running.
    pub async fn checkpoint(&self) -> Result<()> {
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Writes a consistent snapshot to `target` without stopping the daemon.
    pub async fn backup_to(&self, target: impl AsRef<Path>) -> Result<PathBuf> {
        let target = target.as_ref().to_path_buf();
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // VACUUM INTO refuses to overwrite, which is the safety property we want.
        sqlx::query("VACUUM INTO ?")
            .bind(target.to_string_lossy().to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| {
                StoreError::Conflict(format!(
                    "backup to {} failed (does the file already exist?): {e}",
                    target.display()
                ))
            })?;
        Ok(target)
    }
}

/// Appends one event using an existing connection or transaction.
///
/// Callers that mutate state MUST call this inside the same transaction so the event
/// log and the projection can never disagree.
pub(crate) async fn insert_event(
    conn: &mut SqliteConnection,
    event: &jcs_core::NewEvent,
) -> Result<i64> {
    let payload = serde_json::to_string(&event.payload)?;
    let result = sqlx::query(
        "INSERT INTO events (id, ts, kind, project_id, goal_id, task_id, run_id, payload) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(event.id.as_str())
    .bind(Event::encode_ts(event.ts))
    .bind(event.kind.as_str())
    .bind(event.project_id.as_ref().map(|id| id.as_str()))
    .bind(event.goal_id.as_ref().map(|id| id.as_str()))
    .bind(event.task_id.as_ref().map(|id| id.as_str()))
    .bind(event.run_id.as_ref().map(|id| id.as_str()))
    .bind(payload)
    .execute(&mut *conn)
    .await?;

    Ok(result.last_insert_rowid())
}

/// Publishes an event on its own (no accompanying state change).
pub async fn append_event(store: &Store, event: jcs_core::NewEvent) -> Result<i64> {
    let mut conn = store.pool().acquire().await?;
    insert_event(&mut conn, &event).await
}

pub(crate) fn encode_ts(ts: Timestamp) -> String {
    Event::encode_ts(ts)
}

/// SQLite reports unique-constraint violations with these extended result codes.
pub(crate) fn is_unique_violation(error: &dyn sqlx::error::DatabaseError) -> bool {
    matches!(error.code().as_deref(), Some("2067") | Some("1555"))
}

pub(crate) fn decode_ts(value: &str) -> Result<Timestamp> {
    value.parse::<Timestamp>().map_err(|e| StoreError::Corrupt {
        table: "timestamp",
        detail: format!("{value:?}: {e}"),
    })
}
