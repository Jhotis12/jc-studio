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
    instructions: Option<String>,
    created_at: String,
    updated_at: String,
}

impl TaskRow {
    pub(crate) fn into_domain(self) -> Result<Task> {
        Ok(Task {
            id: TaskId::parse(self.id)?,
            goal_id: GoalId::parse(self.goal_id)?,
            title: self.title,
            instructions: self.instructions,
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
        "id, goal_id, title, status, worktree, branch, session_id, role, priority,          path_claims, instructions, created_at, updated_at"
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
            instructions: None,
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
            "INSERT INTO tasks (id, goal_id, title, status, instructions, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(task.id.as_str())
        .bind(task.goal_id.as_str())
        .bind(&task.title)
        .bind(task.status.as_str())
        .bind(task.instructions.as_deref())
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

    /// Replaces the standalone instructions for a task.
    pub async fn set_task_instructions(&self, id: &TaskId, instructions: &str) -> Result<()> {
        sqlx::query("UPDATE tasks SET instructions = ?, updated_at = ? WHERE id = ?")
            .bind(instructions.trim())
            .bind(crate::store::encode_ts(jiff::Timestamp::now()))
            .bind(id.as_str())
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Clears a merge error so the merge queue can try again.
    pub async fn clear_task_merge_error(&self, id: &TaskId) -> Result<()> {
        sqlx::query("UPDATE tasks SET merge_error = NULL, updated_at = ? WHERE id = ?")
            .bind(crate::store::encode_ts(jiff::Timestamp::now()))
            .bind(id.as_str())
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Puts a failed or blocked task back in the queue, optionally with new instructions.
    pub async fn retry_task(&self, id: &TaskId, feedback: Option<&str>) -> Result<Task> {
        let task = self
            .get_task(id)
            .await?
            .ok_or_else(|| StoreError::not_found("task", id))?;
        if !matches!(task.status, TaskStatus::Failed | TaskStatus::Blocked) {
            return Err(StoreError::Conflict(format!(
                "task {id} is {} and cannot be retried",
                task.status.as_str()
            )));
        }
        if let Some(feedback) = feedback.filter(|text| !text.trim().is_empty()) {
            // Keep the plan's original description and append the operator's correction:
            // the next agent needs both the intent and the reason it changed.
            let combined = match task.instructions.as_deref() {
                Some(previous) if !previous.trim().is_empty() => {
                    format!("{previous}\n\nRevision feedback:\n{feedback}")
                }
                _ => feedback.to_string(),
            };
            self.set_task_instructions(id, &combined).await?;
        }
        self.set_task_status(id, TaskStatus::Pending).await?;
        self.clear_task_merge_error(id).await?;
        self.get_task(id)
            .await?
            .ok_or_else(|| StoreError::not_found("task", id))
    }

    /// Creates a follow-up task that depends on a finished one, carrying operator feedback.
    ///
    /// The new task is cut from the goal's integration branch, so it sees the original
    /// commit once that commit has been integrated.
    pub async fn add_iteration_task(&self, original: &Task, feedback: &str) -> Result<Task> {
        let feedback = feedback.trim();
        if feedback.is_empty() {
            return Err(StoreError::Domain(jcs_core::CoreError::EmptyField {
                field: "feedback",
            }));
        }
        let title: String = format!("Iterar: {}", original.title)
            .chars()
            .take(120)
            .collect();
        let now = jiff::Timestamp::now();
        let task = Task {
            id: TaskId::new(),
            goal_id: original.goal_id.clone(),
            title,
            instructions: Some(feedback.to_string()),
            status: TaskStatus::Pending,
            worktree: None,
            branch: None,
            session_id: None,
            role: original.role,
            priority: original.priority,
            path_claims: original.path_claims.clone(),
            created_at: now,
            updated_at: now,
        };

        let mut tx = self.pool().begin().await?;
        let position: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM tasks WHERE goal_id = ?",
        )
        .bind(original.goal_id.as_str())
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO tasks \
             (id, goal_id, title, status, role, priority, path_claims, instructions, position, \
              created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(task.id.as_str())
        .bind(task.goal_id.as_str())
        .bind(&task.title)
        .bind(task.status.as_str())
        .bind(task.role.as_str())
        .bind(task.priority as i64)
        .bind(serde_json::to_string(&task.path_claims)?)
        .bind(task.instructions.as_deref())
        .bind(position)
        .bind(crate::store::encode_ts(now))
        .bind(crate::store::encode_ts(now))
        .execute(&mut *tx)
        .await?;

        sqlx::query("INSERT INTO task_deps (task_id, depends_on) VALUES (?, ?)")
            .bind(task.id.as_str())
            .bind(original.id.as_str())
            .execute(&mut *tx)
            .await?;

        let event = NewEvent::new(EventKind::TaskCreated)
            .goal(&task.goal_id)
            .task(&task.id)
            .payload(serde_json::json!({
                "title": task.title,
                "iteration_of": original.id,
            }));
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

#[cfg(test)]
mod iteration_tests {
    use super::*;
    use jcs_core::{Project, ProjectId};
    use std::time::Duration;

    async fn fixture() -> (Store, jcs_core::Goal, Task) {
        let store = Store::in_memory().await.unwrap();
        let project = Project {
            id: ProjectId::new(),
            name: "iterate".to_string(),
            root_path: std::env::temp_dir().join("jcs-iterate"),
            base_branch: "main".to_string(),
            created_at: jiff::Timestamp::now(),
            auto_merge: false,
            budget_usd_per_day: None,
            budget_tokens_per_day: None,
        };
        store.insert_project(&project).await.unwrap();
        let goal = store
            .create_goal_with_settings(&project.id, "objective", None, None, "auto")
            .await
            .unwrap();
        let task = store.create_task(&goal.id, "original").await.unwrap();
        let _ = Duration::from_secs(1);
        (store, goal, task)
    }

    #[tokio::test]
    async fn iteration_creates_a_dependent_task_with_feedback() {
        let (store, goal, original) = fixture().await;
        store
            .set_task_status(&original.id, TaskStatus::Running)
            .await
            .unwrap();
        store
            .set_task_status(&original.id, TaskStatus::Verifying)
            .await
            .unwrap();
        store
            .set_task_status(&original.id, TaskStatus::Succeeded)
            .await
            .unwrap();

        let iteration = store
            .add_iteration_task(&original, "use a different header")
            .await
            .unwrap();

        assert_eq!(iteration.goal_id, goal.id);
        assert_eq!(iteration.status, TaskStatus::Pending);
        assert_eq!(
            iteration.instructions.as_deref(),
            Some("use a different header")
        );
        let deps = store.task_dependencies(&iteration.id).await.unwrap();
        assert_eq!(deps, vec![original.id]);
    }

    #[tokio::test]
    async fn a_blocked_task_can_be_retried_with_feedback() {
        let (store, _goal, task) = fixture().await;
        store
            .set_task_status(&task.id, TaskStatus::Blocked)
            .await
            .unwrap();
        let retried = store
            .retry_task(&task.id, Some("the secret was in .env"))
            .await
            .unwrap();
        assert_eq!(retried.status, TaskStatus::Pending);
        assert!(retried
            .instructions
            .as_deref()
            .is_some_and(|text| text.contains("the secret was in .env")));
    }
}
