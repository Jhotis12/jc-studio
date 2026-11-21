use jiff::Timestamp;
use sqlx::Row;

use crate::error::{Result, StoreError};
use crate::store::{decode_ts, Store};
use jcs_core::{Event, EventId, EventKind};

fn row_to_event(row: &sqlx::sqlite::SqliteRow) -> Result<Event> {
    let kind: String = row.get("kind");
    let kind = EventKind::parse(&kind)?;
    let payload: String = row.get("payload");
    let payload = serde_json::from_str(&payload).map_err(|e| StoreError::Corrupt {
        table: "events",
        detail: format!("payload is not valid JSON: {e}"),
    })?;

    let id: String = row.get("id");
    let ts: String = row.get("ts");

    Ok(Event {
        seq: row.get("seq"),
        id: EventId::parse(id)?,
        ts: decode_ts(&ts)?,
        kind,
        project_id: row
            .get::<Option<String>, _>("project_id")
            .map(jcs_core::ProjectId::parse)
            .transpose()?,
        goal_id: row
            .get::<Option<String>, _>("goal_id")
            .map(jcs_core::GoalId::parse)
            .transpose()?,
        task_id: row
            .get::<Option<String>, _>("task_id")
            .map(jcs_core::TaskId::parse)
            .transpose()?,
        run_id: row
            .get::<Option<String>, _>("run_id")
            .map(jcs_core::RunId::parse)
            .transpose()?,
        payload,
    })
}

/// Reads events strictly after `since`, oldest first.
///
/// `since` is a lower bound, not a count: gaps in `seq` are expected and callers must
/// not assume contiguity.
pub async fn events_since(store: &Store, since: i64, limit: i64) -> Result<Vec<Event>> {
    let limit = limit.clamp(1, 5_000);
    let rows = sqlx::query(
        "SELECT seq, id, ts, kind, project_id, goal_id, task_id, run_id, payload \
         FROM events WHERE seq > ? ORDER BY seq ASC LIMIT ?",
    )
    .bind(since)
    .bind(limit)
    .fetch_all(store.pool())
    .await?;

    rows.iter().map(row_to_event).collect()
}

/// Reads events for one task, oldest first. Used to rebuild a task timeline.
pub async fn events_for_task(
    store: &Store,
    task_id: &jcs_core::TaskId,
    limit: i64,
) -> Result<Vec<Event>> {
    let limit = limit.clamp(1, 5_000);
    let rows = sqlx::query(
        "SELECT seq, id, ts, kind, project_id, goal_id, task_id, run_id, payload \
         FROM events WHERE task_id = ? ORDER BY seq ASC LIMIT ?",
    )
    .bind(task_id.as_str())
    .bind(limit)
    .fetch_all(store.pool())
    .await?;

    rows.iter().map(row_to_event).collect()
}

/// The most recent `limit` events, oldest first.
///
/// Used by `jcs logs` and to locate a cursor before starting to follow a run.
pub async fn recent_events(store: &Store, limit: i64) -> Result<Vec<Event>> {
    let limit = limit.clamp(1, 5_000);
    let rows = sqlx::query(
        "SELECT seq, id, ts, kind, project_id, goal_id, task_id, run_id, payload FROM ( \
             SELECT seq, id, ts, kind, project_id, goal_id, task_id, run_id, payload \
             FROM events ORDER BY seq DESC LIMIT ? \
         ) ORDER BY seq ASC",
    )
    .bind(limit)
    .fetch_all(store.pool())
    .await?;

    rows.iter().map(row_to_event).collect()
}

/// Highest sequence number written so far, or `0` for an empty log.
pub async fn latest_seq(store: &Store) -> Result<i64> {
    let seq: Option<i64> = sqlx::query_scalar("SELECT MAX(seq) FROM events")
        .fetch_one(store.pool())
        .await?;
    Ok(seq.unwrap_or(0))
}

/// Number of persisted events. For `jcs doctor` and metrics, not for hot paths.
pub async fn count_events(store: &Store) -> Result<i64> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(store.pool())
        .await?;
    Ok(count)
}

/// Timestamp of the newest event, if any.
pub async fn latest_event_ts(store: &Store) -> Result<Option<Timestamp>> {
    let ts: Option<String> = sqlx::query_scalar("SELECT ts FROM events ORDER BY seq DESC LIMIT 1")
        .fetch_optional(store.pool())
        .await?;
    ts.map(|value| decode_ts(&value)).transpose()
}
