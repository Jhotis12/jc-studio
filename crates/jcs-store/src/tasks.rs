use std::path::{Path, PathBuf};

use sqlx::FromRow;

use crate::error::{Result, StoreError};
use crate::store::Store;
use jcs_core::{EventKind, GoalId, NewEvent, ProjectId, StatusMachine, Task, TaskId, TaskStatus};
use jiff::Timestamp;

#[derive(Debug, FromRow)]
pub(crate) struct TaskRow {
    id: String,
    goal_id: String,
    title: String,
    status: String,
    worktree: Option<String>,
    branch: Option<String>,
    session_id: Option<String>,
    role: String,
    priority: i64,
    path_claims: String,
    created_at: String,
    updated_at: String,
}

impl TaskRow {
    pub(crate) fn into_domain(self) -> Result<Task> {
        Ok(Task {
            id: TaskId::parse(self.id)?,
            goal_id: GoalId::parse(self.goal_id)?,
            title: self.title,
            status: TaskStatus::parse(&self.status)?,
            worktree: self.worktree.map(PathBuf::from),
            branch: self.branch,
            session_id: self.session_id,
            role: jcs_core::TaskRole::parse(&self.role)?,
            priority: self.priority as i32,
            path_claims: serde_json::from_str(&self.path_claims)?,
            created_at: crate::store::decode_ts(&self.created_at)?,
            updated_at: crate::store::decode_ts(&self.updated_at)?,
        })
    }
}

macro_rules! task_columns {
    () => {
        "id, goal_id, title, status, worktree, branch, session_id, role, priority,          path_claims, created_at, updated_at"
    };
}

impl Store {
    /// When a task was integrated, without loading the whole task row.
    pub async fn task_merged_at(&self, id: &TaskId) -> Result<Option<Timestamp>> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT merged_at FROM tasks WHERE id = ?")
                .bind(id.as_str())
                .fetch_optional(self.pool())
                .await?;
        match row {
            Some((Some(value),)) => Ok(Some(crate::store::decode_ts(&value)?)),
            _ => Ok(None),
        }
    }

    pub async fn create_task(&self, goal_id: &GoalId, title: &str) -> Result<Task> {
        let title = title.trim();
        if title.is_empty() {
            return Err(StoreError::Domain(jcs_core::CoreError::EmptyField {
                field: "title",
            }));
        }

        if self.get_goal(goal_id).await?.is_none() {
            return Err(StoreError::not_found("goal", goal_id));
        }

        let now = jiff::Timestamp::now();
        let task = Task {
            id: TaskId::new(),
            goal_id: goal_id.clone(),
            title: title.to_string(),
            status: TaskStatus::Pending,
            worktree: None,
            branch: None,
            session_id: None,
            role: jcs_core::TaskRole::Implement,
            priority: 0,
            path_claims: Vec::new(),
            created_at: now,
            updated_at: now,
        };

        let mut tx = self.pool().begin().await?;
        sqlx::query(
            "INSERT INTO tasks (id, goal_id, title, status, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(task.id.as_str())
        .bind(task.goal_id.as_str())
        .bind(&task.title)
        .bind(task.status.as_str())
        .bind(crate::store::encode_ts(task.created_at))
        .bind(crate::store::encode_ts(task.updated_at))
        .execute(&mut *tx)
        .await?;

        let event = NewEvent::new(EventKind::TaskCreated)
            .goal(goal_id)
            .task(&task.id)
            .payload(serde_json::json!({ "title": task.title }));
        crate::store::insert_event(&mut tx, &event).await?;

        tx.commit().await?;
        Ok(task)
    }

    /// Moves a task to `next`, rejecting illegal transitions inside the transaction.
    pub async fn set_task_status(&self, id: &TaskId, next: TaskStatus) -> Result<Task> {
        let mut tx = self.pool().begin().await?;

        let current: Option<String> = sqlx::query_scalar("SELECT status FROM tasks WHERE id = ?")
            .bind(id.as_str())
            .fetch_optional(&mut *tx)
            .await?;
        let current = current.ok_or_else(|| StoreError::not_found("task", id))?;
        let current = TaskStatus::parse(&current)?;
        current.transition(next, "task")?;

        sqlx::query("UPDATE tasks SET status = ?, updated_at = ? WHERE id = ?")
            .bind(next.as_str())
            .bind(crate::store::encode_ts(jiff::Timestamp::now()))
            .bind(id.as_str())
            .execute(&mut *tx)
            .await?;

        let event = NewEvent::new(EventKind::TaskStatusChanged)
            .task(id)
            .payload(serde_json::json!({ "from": current.as_str(), "to": next.as_str() }));
        crate::store::insert_event(&mut tx, &event).await?;

        tx.commit().await?;
        self.get_task(id)
            .await?
            .ok_or_else(|| StoreError::not_found("task", id))
    }

    /// Records the isolated worktree a task's agent is confined to.
    pub async fn attach_worktree(
        &self,
        id: &TaskId,
        worktree: &Path,
        branch: &str,
    ) -> Result<Task> {
        let mut tx = self.pool().begin().await?;

        let result =
            sqlx::query("UPDATE tasks SET worktree = ?, branch = ?, updated_at = ? WHERE id = ?")
                .bind(worktree.to_string_lossy().to_string())
                .bind(branch)
                .bind(crate::store::encode_ts(jiff::Timestamp::now()))
                .bind(id.as_str())
                .execute(&mut *tx)
                .await?;

        if result.rows_affected() == 0 {
            return Err(StoreError::not_found("task", id));
        }

        let event = NewEvent::new(EventKind::TaskWorktreeCreated)
            .task(id)
            .payload(serde_json::json!({
                "worktree": worktree.to_string_lossy(),
                "branch": branch,
            }));
        crate::store::insert_event(&mut tx, &event).await?;

        tx.commit().await?;
        self.get_task(id)
            .await?
            .ok_or_else(|| StoreError::not_found("task", id))
    }

    /// Binds an agent session to a task and remembers where it lives on disk.
    pub async fn link_session(
        &self,
        task_id: &TaskId,
        project_id: &ProjectId,
        session_id: &str,
        directory: &Path,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;

        sqlx::query(
            "INSERT INTO sessions_map (session_id, task_id, project_id, directory, created_at) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT(session_id) DO UPDATE SET task_id = excluded.task_id",
        )
        .bind(session_id)
        .bind(task_id.as_str())
        .bind(project_id.as_str())
        .bind(directory.to_string_lossy().to_string())
        .bind(crate::store::encode_ts(jiff::Timestamp::now()))
        .execute(&mut *tx)
        .await?;

        sqlx::query("UPDATE tasks SET session_id = ?, updated_at = ? WHERE id = ?")
            .bind(session_id)
            .bind(crate::store::encode_ts(jiff::Timestamp::now()))
            .bind(task_id.as_str())
            .execute(&mut *tx)
            .await?;

        let event = NewEvent::new(EventKind::AgentSessionLinked)
            .task(task_id)
            .project(project_id)
            .payload(serde_json::json!({
                "session_id": session_id,
                "directory": directory.to_string_lossy(),
            }));
        crate::store::insert_event(&mut tx, &event).await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn get_task(&self, id: &TaskId) -> Result<Option<Task>> {
        let row: Option<TaskRow> = sqlx::query_as(concat!(
            "SELECT ",
            task_columns!(),
            " FROM tasks WHERE id = ?"
        ))
        .bind(id.as_str())
        .fetch_optional(self.pool())
        .await?;

        row.map(TaskRow::into_domain).transpose()
    }

    pub async fn list_tasks_by_goal(&self, goal_id: &GoalId) -> Result<Vec<Task>> {
        let rows: Vec<TaskRow> = sqlx::query_as(concat!(
            "SELECT ",
            task_columns!(),
            " FROM tasks WHERE goal_id = ? ORDER BY created_at"
        ))
        .bind(goal_id.as_str())
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(TaskRow::into_domain).collect()
    }

    pub async fn list_tasks(&self, limit: i64) -> Result<Vec<Task>> {
        let rows: Vec<TaskRow> = sqlx::query_as(concat!(
            "SELECT ",
            task_columns!(),
            " FROM tasks ORDER BY created_at DESC LIMIT ?"
        ))
        .bind(limit.clamp(1, 1_000))
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(TaskRow::into_domain).collect()
    }

    /// Tasks that were in flight when the daemon stopped. Recovery input.
    pub async fn recoverable_tasks(&self) -> Result<Vec<Task>> {
        let rows: Vec<TaskRow> = sqlx::query_as(concat!(
            "SELECT ",
            task_columns!(),
            " FROM tasks WHERE status IN ('running', 'verifying') \
             ORDER BY created_at"
        ))
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(TaskRow::into_domain).collect()
    }

    /// Parks tasks that were mid-flight when the daemon died.
    ///
    /// `Blocked` rather than `Failed` on purpose: a killed process proves nothing about
    /// the work, so the task must not be counted as a failure nor silently retried in a
    /// loop. It stays visible in `jcs status` until a human or the next goal touches it.
    pub async fn block_orphaned_tasks(&self, reason: &str) -> Result<usize> {
        let orphaned = self.recoverable_tasks().await?;
        let mut count = 0;

        for task in orphaned {
            // Both `Running` and `Verifying` can reach `Blocked` directly. Going through
            // `Failed` first is illegal by design (a killed process is not a failure) and
            // made recovery abort daemon startup.
            self.set_task_status(&task.id, TaskStatus::Blocked).await?;

            let event = NewEvent::new(EventKind::Error)
                .task(&task.id)
                .payload(serde_json::json!({
                    "reason": reason,
                    "previous_status": task.status.as_str(),
                    "recovery": "task parked as blocked; inspect with `jcs status`",
                }));
            crate::append_event(self, event).await?;
            count += 1;
        }

        Ok(count)
    }

    /// Project that owns a task, via its goal.
    pub async fn project_for_task(&self, task_id: &TaskId) -> Result<Option<ProjectId>> {
        let id: Option<String> = sqlx::query_scalar(
            "SELECT g.project_id FROM tasks t JOIN goals g ON g.id = t.goal_id WHERE t.id = ?",
        )
        .bind(task_id.as_str())
        .fetch_optional(self.pool())
        .await?;

        id.map(ProjectId::parse).transpose().map_err(Into::into)
    }

    /// Total number of tasks, independent of any list limit.
    pub async fn task_count(&self) -> Result<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks")
            .fetch_one(self.pool())
            .await?;
        Ok(count)
    }

    /// Whether the task has any worktree recorded yet.
    pub async fn task_has_worktree(&self, task_id: &TaskId) -> Result<bool> {
        let worktree: Option<Option<String>> =
            sqlx::query_scalar("SELECT worktree FROM tasks WHERE id = ?")
                .bind(task_id.as_str())
                .fetch_optional(self.pool())
                .await?;

        Ok(worktree.flatten().is_some())
    }
}
