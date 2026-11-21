//! Integration branches: the backbone of a goal's DAG.
//!
//! Every task worktree is cut from its goal's integration branch, and every succeeded task
//! is merged back into it. That is what makes `depends_on` meaningful: a dependent task
//! starts from code that already contains its predecessor's commit.
//!
//! When the project has `auto_merge` enabled, the integration branch is fast-forwarded into
//! the base branch once the whole goal succeeds. Otherwise it remains for a human to merge.

use crate::error::Result;
use crate::store::Store;
use jcs_core::{EventKind, GoalId, NewEvent, Task, TaskId};

impl Store {
    pub async fn set_goal_integration_branch(&self, goal_id: &GoalId, branch: &str) -> Result<()> {
        sqlx::query("UPDATE goals SET integration_branch = ? WHERE id = ?")
            .bind(branch)
            .bind(goal_id.as_str())
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn goal_integration_branch(&self, goal_id: &GoalId) -> Result<Option<String>> {
        let branch: Option<Option<String>> =
            sqlx::query_scalar("SELECT integration_branch FROM goals WHERE id = ?")
                .bind(goal_id.as_str())
                .fetch_optional(self.pool())
                .await?;
        Ok(branch.flatten())
    }

    pub async fn goal_base_branch(&self, goal_id: &GoalId) -> Result<Option<String>> {
        let branch: Option<String> = sqlx::query_scalar(
            "SELECT p.base_branch FROM goals g JOIN projects p ON p.id = g.project_id \
             WHERE g.id = ?",
        )
        .bind(goal_id.as_str())
        .fetch_optional(self.pool())
        .await?;
        Ok(branch)
    }

    /// Records that a task's commit is now part of its goal's integration branch.
    pub async fn mark_task_merged(&self, task_id: &TaskId, sha: &str) -> Result<()> {
        sqlx::query("UPDATE tasks SET merged_at = ?, merge_error = NULL WHERE id = ?")
            .bind(crate::store::encode_ts(jiff::Timestamp::now()))
            .bind(task_id.as_str())
            .execute(self.pool())
            .await?;

        let task = self.get_task(task_id).await?;
        let mut event = NewEvent::new(EventKind::MergeFinished)
            .task(task_id)
            .payload(serde_json::json!({ "sha": sha, "result": "merged" }));
        if let Some(task) = &task {
            event = event.goal(&task.goal_id);
        }
        crate::append_event(self, event).await?;
        Ok(())
    }

    pub async fn mark_task_merge_failed(&self, task_id: &TaskId, error: &str) -> Result<()> {
        sqlx::query("UPDATE tasks SET merge_error = ? WHERE id = ?")
            .bind(error)
            .bind(task_id.as_str())
            .execute(self.pool())
            .await?;

        let task = self.get_task(task_id).await?;
        let mut event = NewEvent::new(EventKind::MergeFinished)
            .task(task_id)
            .payload(serde_json::json!({ "result": "failed", "error": error }));
        if let Some(task) = &task {
            event = event.goal(&task.goal_id);
        }
        crate::append_event(self, event).await?;
        Ok(())
    }

    /// Succeeded tasks whose commit is not yet part of the integration branch.
    pub async fn tasks_pending_merge(&self) -> Result<Vec<Task>> {
        let rows: Vec<crate::tasks::TaskRow> = sqlx::query_as(
            "SELECT t.id, t.goal_id, t.title, t.status, t.worktree, t.branch, t.session_id, \
                    t.role, t.priority, t.path_claims, t.created_at, t.updated_at \
             FROM tasks t JOIN goals g ON g.id = t.goal_id \
             WHERE t.status = 'succeeded' AND t.merged_at IS NULL AND t.merge_error IS NULL \
               AND g.integration_branch IS NOT NULL \
             ORDER BY t.position, t.created_at",
        )
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(|row| row.into_domain()).collect()
    }

    /// Clears merge failures so a resumed goal retries the merge.
    pub async fn clear_goal_merge_errors(&self, goal_id: &GoalId) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE tasks SET merge_error = NULL WHERE goal_id = ? AND merge_error IS NOT NULL",
        )
        .bind(goal_id.as_str())
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected())
    }

    /// How many succeeded tasks of the goal are not yet on its integration branch.
    pub async fn goal_pending_merges(&self, goal_id: &GoalId) -> Result<i64> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tasks t JOIN goals g ON g.id = t.goal_id \
             WHERE t.goal_id = ? AND t.status = 'succeeded' AND t.merged_at IS NULL \
               AND t.merge_error IS NULL AND g.integration_branch IS NOT NULL",
        )
        .bind(goal_id.as_str())
        .fetch_one(self.pool())
        .await?;
        Ok(count)
    }

    /// Whether any task of the goal failed to merge.
    pub async fn goal_has_merge_errors(&self, goal_id: &GoalId) -> Result<bool> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tasks WHERE goal_id = ? AND merge_error IS NOT NULL",
        )
        .bind(goal_id.as_str())
        .fetch_one(self.pool())
        .await?;
        Ok(count > 0)
    }

    pub async fn task_merge_error(&self, task_id: &TaskId) -> Result<Option<String>> {
        let error: Option<Option<String>> =
            sqlx::query_scalar("SELECT merge_error FROM tasks WHERE id = ?")
                .bind(task_id.as_str())
                .fetch_optional(self.pool())
                .await?;
        Ok(error.flatten())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jcs_core::{Plan, ProjectId, TaskRole, TaskSpec, TaskStatus};

    async fn fixture() -> (Store, GoalId, Vec<Task>) {
        let store = Store::in_memory().await.unwrap();
        let project = jcs_core::Project {
            id: ProjectId::new(),
            name: "demo".into(),
            root_path: std::path::PathBuf::from("/tmp/demo"),
            base_branch: "main".into(),
            created_at: jiff::Timestamp::now(),
            auto_merge: true,
            budget_usd_per_day: None,
            budget_tokens_per_day: None,
        };
        store.insert_project(&project).await.unwrap();
        let goal = store.create_goal(&project.id, "objective").await.unwrap();

        let plan = Plan {
            tasks: vec![
                TaskSpec {
                    key: "a".into(),
                    title: "a".into(),
                    description: "a".into(),
                    depends_on: vec![],
                    paths: vec!["src/a".into()],
                    role: TaskRole::Implement,
                },
                TaskSpec {
                    key: "b".into(),
                    title: "b".into(),
                    description: "b".into(),
                    depends_on: vec!["a".into()],
                    paths: vec!["src/b".into()],
                    role: TaskRole::Implement,
                },
            ],
        };
        let raw = serde_json::to_string(&plan).unwrap();
        let tasks = store
            .create_planned_tasks(&goal.id, &plan, &raw)
            .await
            .unwrap();
        (store, goal.id, tasks)
    }

    #[tokio::test]
    async fn integration_branch_is_stored_on_the_goal() {
        let (store, goal, _) = fixture().await;
        assert!(store
            .goal_integration_branch(&goal)
            .await
            .unwrap()
            .is_none());

        store
            .set_goal_integration_branch(&goal, "jcs/integration/abc")
            .await
            .unwrap();
        assert_eq!(
            store
                .goal_integration_branch(&goal)
                .await
                .unwrap()
                .as_deref(),
            Some("jcs/integration/abc")
        );
        assert_eq!(
            store.goal_base_branch(&goal).await.unwrap().as_deref(),
            Some("main")
        );
    }

    #[tokio::test]
    async fn only_succeeded_unmerged_tasks_are_pending_merge() {
        let (store, goal, tasks) = fixture().await;
        store
            .set_goal_integration_branch(&goal, "jcs/integration/abc")
            .await
            .unwrap();

        // Nothing succeeded yet.
        assert!(store.tasks_pending_merge().await.unwrap().is_empty());

        let first = &tasks[0];
        store
            .set_task_status(&first.id, TaskStatus::Running)
            .await
            .unwrap();
        store
            .set_task_status(&first.id, TaskStatus::Verifying)
            .await
            .unwrap();
        store
            .set_task_status(&first.id, TaskStatus::Succeeded)
            .await
            .unwrap();

        let pending = store.tasks_pending_merge().await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, first.id);

        store.mark_task_merged(&first.id, "abc123").await.unwrap();
        assert!(store.tasks_pending_merge().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn merge_failures_are_recorded_without_losing_the_task() {
        let (store, _goal, tasks) = fixture().await;
        let task = &tasks[0];

        store
            .set_task_status(&task.id, TaskStatus::Running)
            .await
            .unwrap();
        store
            .set_task_status(&task.id, TaskStatus::Verifying)
            .await
            .unwrap();
        store
            .set_task_status(&task.id, TaskStatus::Succeeded)
            .await
            .unwrap();

        store
            .mark_task_merge_failed(&task.id, "conflict in src/a.rs")
            .await
            .unwrap();

        assert_eq!(
            store.task_merge_error(&task.id).await.unwrap().as_deref(),
            Some("conflict in src/a.rs")
        );
        // The task is still there and still succeeded: the work is valid, the merge is not.
        assert_eq!(
            store.get_task(&task.id).await.unwrap().unwrap().status,
            TaskStatus::Succeeded
        );
    }
}
