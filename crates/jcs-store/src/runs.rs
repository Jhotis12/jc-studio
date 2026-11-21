use sqlx::FromRow;

use crate::error::{Result, StoreError};
use crate::store::Store;
use jcs_core::{
    EventKind, GateStatus, NewEvent, Run, RunId, RunStatus, StatusMachine, TaskId,
};

#[derive(Debug, FromRow)]
struct RunRow {
    id: String,
    task_id: String,
    attempt: i64,
    status: String,
    exit_code: Option<i64>,
    started_at: String,
    ended_at: Option<String>,
    cost_usd: f64,
    tokens_in: i64,
    tokens_out: i64,
    error: Option<String>,
}

impl RunRow {
    fn into_domain(self) -> Result<Run> {
        Ok(Run {
            id: RunId::parse(self.id)?,
            task_id: TaskId::parse(self.task_id)?,
            attempt: u32::try_from(self.attempt).unwrap_or(u32::MAX),
            status: RunStatus::parse(&self.status)?,
            exit_code: self.exit_code.map(|code| code as i32),
            started_at: crate::store::decode_ts(&self.started_at)?,
            ended_at: self
                .ended_at
                .as_deref()
                .map(crate::store::decode_ts)
                .transpose()?,
            cost_usd: self.cost_usd,
            tokens_in: self.tokens_in,
            tokens_out: self.tokens_out,
            error: self.error,
        })
    }
}

const RUN_COLUMNS: &str = "id, task_id, attempt, status, exit_code, started_at, ended_at, \
                           cost_usd, tokens_in, tokens_out, error";

impl Store {
    /// Starts a new attempt for a task. Attempt numbers are dense per task.
    pub async fn start_run(&self, task_id: &TaskId) -> Result<Run> {
        let mut tx = self.pool().begin().await?;

        let goal_id: Option<String> =
            sqlx::query_scalar("SELECT goal_id FROM tasks WHERE id = ?")
                .bind(task_id.as_str())
                .fetch_optional(&mut *tx)
                .await?;
        let goal_id = goal_id.ok_or_else(|| StoreError::not_found("task", task_id))?;

        let attempt: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(attempt), 0) + 1 FROM runs WHERE task_id = ?",
        )
        .bind(task_id.as_str())
        .fetch_one(&mut *tx)
        .await?;

        let run = Run {
            id: RunId::new(),
            task_id: task_id.clone(),
            attempt: u32::try_from(attempt).unwrap_or(u32::MAX),
            status: RunStatus::Started,
            exit_code: None,
            started_at: chrono::Utc::now(),
            ended_at: None,
            cost_usd: 0.0,
            tokens_in: 0,
            tokens_out: 0,
            error: None,
        };

        sqlx::query(
            "INSERT INTO runs (id, task_id, attempt, status, started_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(run.id.as_str())
        .bind(run.task_id.as_str())
        .bind(i64::from(run.attempt))
        .bind(run.status.as_str())
        .bind(crate::store::encode_ts(run.started_at))
        .execute(&mut *tx)
        .await?;

        let event = NewEvent::new(EventKind::RunStarted)
            .goal(&jcs_core::GoalId::parse(goal_id)?)
            .task(task_id)
            .run(&run.id)
            .payload(serde_json::json!({ "attempt": run.attempt }));
        crate::store::insert_event(&mut tx, &event).await?;

        tx.commit().await?;
        Ok(run)
    }

    /// Replaces the usage counters with the cumulative values reported by the backend.
    ///
    /// Absolute rather than additive on purpose: agent backends report running totals,
    /// and adding those up would double-count every event.
    pub async fn set_run_usage(
        &self,
        run_id: &RunId,
        cost_usd: f64,
        tokens_in: i64,
        tokens_out: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE runs SET cost_usd = ?, tokens_in = ?, tokens_out = ? \
             WHERE id = ? AND status = 'started'",
        )
        .bind(cost_usd.max(0.0))
        .bind(tokens_in.max(0))
        .bind(tokens_out.max(0))
        .bind(run_id.as_str())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Finishes a run. Only a started run can be finished.
    pub async fn finish_run(
        &self,
        run_id: &RunId,
        status: RunStatus,
        exit_code: Option<i32>,
        error: Option<String>,
    ) -> Result<Run> {
        let mut tx = self.pool().begin().await?;

        let row: Option<(String, String, String)> =
            sqlx::query_as("SELECT status, task_id, started_at FROM runs WHERE id = ?")
                .bind(run_id.as_str())
                .fetch_optional(&mut *tx)
                .await?;
        let (current, task_id, _started_at) = row.ok_or_else(|| StoreError::not_found("run", run_id))?;
        let current = RunStatus::parse(&current)?;
        current.transition(status, "run")?;

        sqlx::query(
            "UPDATE runs SET status = ?, exit_code = ?, ended_at = ?, error = ? WHERE id = ?",
        )
        .bind(status.as_str())
        .bind(exit_code.map(i64::from))
        .bind(crate::store::encode_ts(chrono::Utc::now()))
        .bind(error.as_deref())
        .bind(run_id.as_str())
        .execute(&mut *tx)
        .await?;

        let run = self
            .get_run_in(&mut *tx, run_id)
            .await?
            .ok_or_else(|| StoreError::not_found("run", run_id))?;

        let event = NewEvent::new(EventKind::RunFinished)
            .task(&TaskId::parse(task_id)?)
            .run(run_id)
            .payload(serde_json::json!({
                "status": status.as_str(),
                "exit_code": exit_code,
                "cost_usd": run.cost_usd,
                "tokens_in": run.tokens_in,
                "tokens_out": run.tokens_out,
                "error": error,
            }));
        crate::store::insert_event(&mut tx, &event).await?;

        tx.commit().await?;
        Ok(run)
    }

    pub async fn get_run(&self, id: &RunId) -> Result<Option<Run>> {
        let row: Option<RunRow> =
            sqlx::query_as(&format!("SELECT {RUN_COLUMNS} FROM runs WHERE id = ?"))
                .bind(id.as_str())
                .fetch_optional(self.pool())
                .await?;

        row.map(RunRow::into_domain).transpose()
    }

    async fn get_run_in(
        &self,
        conn: &mut sqlx::SqliteConnection,
        id: &RunId,
    ) -> Result<Option<Run>> {
        let row: Option<RunRow> =
            sqlx::query_as(&format!("SELECT {RUN_COLUMNS} FROM runs WHERE id = ?"))
                .bind(id.as_str())
                .fetch_optional(&mut *conn)
                .await?;

        row.map(RunRow::into_domain).transpose()
    }

    pub async fn list_runs_by_task(&self, task_id: &TaskId) -> Result<Vec<Run>> {
        let rows: Vec<RunRow> = sqlx::query_as(&format!(
            "SELECT {RUN_COLUMNS} FROM runs WHERE task_id = ? ORDER BY attempt"
        ))
        .bind(task_id.as_str())
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(RunRow::into_domain).collect()
    }

    pub async fn list_runs(&self, limit: i64) -> Result<Vec<Run>> {
        let rows: Vec<RunRow> = sqlx::query_as(&format!(
            "SELECT {RUN_COLUMNS} FROM runs ORDER BY started_at DESC LIMIT ?"
        ))
        .bind(limit.clamp(1, 1_000))
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(RunRow::into_domain).collect()
    }

    /// Marks every still-running run as interrupted.
    ///
    /// Called at daemon startup: a `started` row on boot means the previous process died
    /// mid-run. `Interrupted` is honest (we do not know the outcome) and retryable.
    pub async fn interrupt_stale_runs(&self, reason: &str) -> Result<usize> {
        let mut tx = self.pool().begin().await?;

        let stale: Vec<(String, String)> =
            sqlx::query_as("SELECT id, task_id FROM runs WHERE status = 'started'")
                .fetch_all(&mut *tx)
                .await?;

        for (run_id, task_id) in &stale {
            sqlx::query(
                "UPDATE runs SET status = 'interrupted', ended_at = ?, error = ? WHERE id = ?",
            )
            .bind(crate::store::encode_ts(chrono::Utc::now()))
            .bind(reason)
            .bind(run_id)
            .execute(&mut *tx)
            .await?;

            let event = NewEvent::new(EventKind::RunFinished)
                .task(&TaskId::parse(task_id.clone())?)
                .run(&RunId::parse(run_id.clone())?)
                .payload(serde_json::json!({
                    "status": "interrupted",
                    "reason": reason,
                }));
            crate::store::insert_event(&mut tx, &event).await?;
        }

        tx.commit().await?;
        Ok(stale.len())
    }

    /// Total spend in US dollars, optionally scoped to one project.
    pub async fn total_cost_usd(&self, project_id: Option<&jcs_core::ProjectId>) -> Result<f64> {
        let total: Option<f64> = match project_id {
            Some(id) => {
                sqlx::query_scalar(
                    "SELECT SUM(r.cost_usd) FROM runs r \
                     JOIN tasks t ON t.id = r.task_id \
                     JOIN goals g ON g.id = t.goal_id \
                     WHERE g.project_id = ?",
                )
                .bind(id.as_str())
                .fetch_one(self.pool())
                .await?
            }
            None => {
                sqlx::query_scalar("SELECT SUM(cost_usd) FROM runs")
                    .fetch_one(self.pool())
                    .await?
            }
        };

        Ok(total.unwrap_or(0.0))
    }
}

#[derive(Debug, FromRow)]
struct GateResultRow {
    name: String,
    status: String,
    exit_code: Option<i64>,
    duration_ms: i64,
}

/// One recorded gate verdict.
#[derive(Debug, Clone, PartialEq)]
pub struct GateResult {
    pub name: String,
    pub status: GateStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: i64,
}

impl Store {
    pub async fn add_gate_result(
        &self,
        run_id: &RunId,
        name: &str,
        status: GateStatus,
        exit_code: Option<i32>,
        duration_ms: i64,
        output_excerpt: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO gate_results \
             (id, run_id, gate_id, name, status, exit_code, duration_ms, output_excerpt, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(jcs_core::EventId::new().as_str())
        .bind(run_id.as_str())
        .bind(name)
        .bind(name)
        .bind(status.as_str())
        .bind(exit_code.map(i64::from))
        .bind(duration_ms.max(0))
        .bind(output_excerpt.map(|s| truncate_chars(s, 8_000)))
        .bind(crate::store::encode_ts(chrono::Utc::now()))
        .execute(self.pool())
        .await?;

        let event = NewEvent::new(EventKind::GateFinished)
            .run(run_id)
            .payload(serde_json::json!({
                "gate": name,
                "status": status.as_str(),
                "exit_code": exit_code,
                "duration_ms": duration_ms,
            }));
        crate::store::append_event(self, event).await?;

        Ok(())
    }

    pub async fn list_gate_results(&self, run_id: &RunId) -> Result<Vec<GateResult>> {
        let rows: Vec<GateResultRow> = sqlx::query_as(
            "SELECT name, status, exit_code, duration_ms FROM gate_results \
             WHERE run_id = ? ORDER BY created_at",
        )
        .bind(run_id.as_str())
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(GateResult {
                    name: row.name,
                    status: GateStatus::parse(&row.status)?,
                    exit_code: row.exit_code.map(|code| code as i32),
                    duration_ms: row.duration_ms,
                })
            })
            .collect()
    }

    /// Whether every recorded gate for a run passed.
    ///
    /// An empty set is *not* success: "no gates ran" must never be reported as green.
    pub async fn all_gates_passed(&self, run_id: &RunId) -> Result<bool> {
        let results = self.list_gate_results(run_id).await?;
        Ok(!results.is_empty() && results.iter().all(|gate| gate.status == GateStatus::Passed))
    }
}

fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut out: String = value.chars().take(max).collect();
    out.push_str("\n… (truncated)");
    out
}
