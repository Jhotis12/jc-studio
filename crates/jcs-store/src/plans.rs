//! Plan persistence and DAG queries.
//!
//! A plan is stored verbatim for audit, and materialized into `tasks` plus `task_deps`.
//! Readiness is a SQL query rather than an in-memory graph: the scheduler must be able to
//! ask "what can run now?" after a restart, with no state of its own.

use std::collections::BTreeMap;

use jiff::Timestamp;

use crate::error::{Result, StoreError};
use crate::store::Store;
use jcs_core::{EventKind, GoalId, NewEvent, Plan, StatusMachine, Task, TaskId, TaskStatus};

/// Task counts by status, used to decide whether a goal is finished.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TaskCounts {
    pub total: i64,
    pub pending: i64,
    pub running: i64,
    pub verifying: i64,
    pub succeeded: i64,
    pub failed: i64,
    pub blocked: i64,
    pub cancelled: i64,
}

impl TaskCounts {
    pub fn in_flight(self) -> i64 {
        self.running + self.verifying
    }

    /// Whether no task can make progress without a human or a new dispatch.
    pub fn settled(self) -> bool {
        self.pending == 0 && self.in_flight() == 0
    }
}

impl Store {
    /// Materializes a validated plan as tasks plus dependency edges, in one transaction.
    pub async fn create_planned_tasks(
        &self,
        goal_id: &GoalId,
        plan: &Plan,
        raw: &str,
    ) -> Result<Vec<Task>> {
        plan.validate()
            .map_err(|error| StoreError::Conflict(error.to_string()))?;

        let goal = self
            .get_goal(goal_id)
            .await?
            .ok_or_else(|| StoreError::not_found("goal", goal_id))?;

        let ordered = plan.topological();
        let mut tx = self.pool().begin().await?;
        let mut ids: BTreeMap<String, TaskId> = BTreeMap::new();
        let now = Timestamp::now();

        for (position, spec) in ordered.iter().enumerate() {
            let id = TaskId::new();
            let claims = serde_json::to_string(&spec.paths)?;

            sqlx::query(
                "INSERT INTO tasks \
                 (id, goal_id, title, status, role, priority, path_claims, position, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id.as_str())
            .bind(goal_id.as_str())
            .bind(&spec.title)
            .bind(TaskStatus::Pending.as_str())
            .bind(spec.role.as_str())
            .bind(0_i64)
            .bind(claims)
            .bind(position as i64)
            .bind(crate::store::encode_ts(now))
            .bind(crate::store::encode_ts(now))
            .execute(&mut *tx)
            .await?;

            ids.insert(spec.key.clone(), id);
        }

        for spec in &plan.tasks {
            let task_id = &ids[&spec.key];
            for dependency in &spec.depends_on {
                sqlx::query("INSERT INTO task_deps (task_id, depends_on) VALUES (?, ?)")
                    .bind(task_id.as_str())
                    .bind(ids[dependency].as_str())
                    .execute(&mut *tx)
                    .await?;
            }
        }

        sqlx::query("INSERT INTO plans (goal_id, version, raw, created_at) VALUES (?, ?, ?, ?)")
            .bind(goal_id.as_str())
            .bind(1_i64)
            .bind(raw)
            .bind(crate::store::encode_ts(now))
            .execute(&mut *tx)
            .await?;

        for spec in &plan.tasks {
            let event = NewEvent::new(EventKind::TaskCreated)
                .project(&goal.project_id)
                .goal(goal_id)
                .task(&ids[&spec.key])
                .payload(serde_json::json!({
                    "key": spec.key,
                    "title": spec.title,
                    "role": spec.role.as_str(),
                    "depends_on": spec.depends_on,
                    "paths": spec.paths,
                }));
            crate::store::insert_event(&mut tx, &event).await?;
        }

        let plan_event = NewEvent::new(EventKind::PlanCreated)
            .project(&goal.project_id)
            .goal(goal_id)
            .payload(serde_json::json!({
                "tasks": plan.tasks.len(),
                "leaves": plan.leaves(),
            }));
        crate::store::insert_event(&mut tx, &plan_event).await?;

        tx.commit().await?;

        self.list_tasks_by_goal(goal_id).await
    }

    /// The plan a goal was created from, if it was planned.
    pub async fn goal_plan(&self, goal_id: &GoalId) -> Result<Option<Plan>> {
        let raw: Option<String> = sqlx::query_scalar("SELECT raw FROM plans WHERE goal_id = ?")
            .bind(goal_id.as_str())
            .fetch_optional(self.pool())
            .await?;

        match raw {
            None => Ok(None),
            Some(raw) => {
                let plan: Plan = serde_json::from_str(&raw)?;
                Ok(Some(plan))
            }
        }
    }

    pub async fn task_dependencies(&self, task_id: &TaskId) -> Result<Vec<TaskId>> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT depends_on FROM task_deps WHERE task_id = ? ORDER BY depends_on",
        )
        .bind(task_id.as_str())
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(|row| TaskId::parse(row).map_err(StoreError::from))
            .collect()
    }

    /// Tasks that can run right now: pending, with every dependency succeeded.
    ///
    /// A dependency must be both succeeded *and* merged into the goal's integration
    /// branch before its dependents can start, otherwise a dependent would be cut from an
    /// integration branch that does not contain the code it is supposed to build on.
    ///
    /// A task whose dependency failed or is blocked never becomes ready: that branch of
    /// the graph is dead until a human intervenes.
    pub async fn ready_tasks(&self, goal_id: &GoalId) -> Result<Vec<Task>> {
        let rows: Vec<crate::tasks::TaskRow> = sqlx::query_as(
            "SELECT t.id, t.goal_id, t.title, t.status, t.worktree, t.branch, t.session_id, \
                    t.created_at, t.updated_at, t.role, t.priority, t.path_claims, t.position \
             FROM tasks t \
             WHERE t.goal_id = ? AND t.status = 'pending' \
               AND NOT EXISTS ( \
                   SELECT 1 FROM task_deps d \
                   JOIN tasks dep ON dep.id = d.depends_on \
                   WHERE d.task_id = t.id \
                     AND ( \
                         dep.status <> 'succeeded' \
                         OR ( \
                             dep.merged_at IS NULL \
                             AND EXISTS ( \
                                 SELECT 1 FROM goals g \
                                 WHERE g.id = t.goal_id AND g.integration_branch IS NOT NULL \
                             ) \
                         ) \
                     ) \
               ) \
             ORDER BY t.priority DESC, t.position ASC",
        )
        .bind(goal_id.as_str())
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(|row| row.into_domain()).collect()
    }

    pub async fn goal_task_counts(&self, goal_id: &GoalId) -> Result<TaskCounts> {
        let rows: Vec<(String, i64)> =
            sqlx::query_as("SELECT status, COUNT(*) FROM tasks WHERE goal_id = ? GROUP BY status")
                .bind(goal_id.as_str())
                .fetch_all(self.pool())
                .await?;

        let mut counts = TaskCounts::default();
        for (status, count) in rows {
            let status = TaskStatus::parse(&status)?;
            counts.total += count;
            match status {
                TaskStatus::Pending => counts.pending = count,
                TaskStatus::Running => counts.running = count,
                TaskStatus::Verifying => counts.verifying = count,
                TaskStatus::Succeeded => counts.succeeded = count,
                TaskStatus::Failed => counts.failed = count,
                TaskStatus::Blocked => counts.blocked = count,
                TaskStatus::Cancelled => counts.cancelled = count,
            }
        }
        Ok(counts)
    }

    /// Tasks of one project that are currently executing, with their path claims.
    pub async fn in_flight_tasks(&self, project_id: &jcs_core::ProjectId) -> Result<Vec<Task>> {
        let rows: Vec<crate::tasks::TaskRow> = sqlx::query_as(
            "SELECT t.id, t.goal_id, t.title, t.status, t.worktree, t.branch, t.session_id, \
                    t.created_at, t.updated_at, t.role, t.priority, t.path_claims, t.position \
             FROM tasks t JOIN goals g ON g.id = t.goal_id \
             WHERE g.project_id = ? AND t.status IN ('running', 'verifying') \
             ORDER BY t.created_at",
        )
        .bind(project_id.as_str())
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(|row| row.into_domain()).collect()
    }

    /// Goals that still have work that could progress.
    pub async fn active_goals(&self) -> Result<Vec<jcs_core::Goal>> {
        let rows: Vec<crate::goals::GoalRow> = sqlx::query_as(
            "SELECT id, project_id, prompt, status, created_at, updated_at \
             FROM goals WHERE status IN ('pending', 'running') ORDER BY created_at, rowid",
        )
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(|row| row.into_domain()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jcs_core::{Plan, ProjectId, TaskRole, TaskSpec};

    fn spec(key: &str, deps: &[&str]) -> TaskSpec {
        TaskSpec {
            key: key.into(),
            title: format!("task {key}"),
            description: format!("do {key}"),
            depends_on: deps.iter().map(|dep| dep.to_string()).collect(),
            paths: vec![format!("src/{key}.rs")],
            role: TaskRole::Implement,
        }
    }

    async fn fixture(plan: Plan) -> (Store, GoalId) {
        let store = Store::in_memory().await.unwrap();
        let project = jcs_core::Project {
            id: ProjectId::new(),
            name: "demo".into(),
            root_path: std::path::PathBuf::from("/tmp/demo"),
            base_branch: "main".into(),
            created_at: Timestamp::now(),
            auto_merge: false,
            budget_usd_per_day: None,
            budget_tokens_per_day: None,
        };
        store.insert_project(&project).await.unwrap();
        let goal = store.create_goal(&project.id, "objective").await.unwrap();
        let raw = serde_json::to_string(&plan).unwrap();
        store
            .create_planned_tasks(&goal.id, &plan, &raw)
            .await
            .unwrap();
        (store, goal.id)
    }

    #[tokio::test]
    async fn a_plan_materializes_into_tasks_with_dependencies() {
        let plan = Plan {
            tasks: vec![spec("a", &[]), spec("b", &["a"]), spec("c", &["a"])],
        };
        let (store, goal) = fixture(plan.clone()).await;

        let tasks = store.list_tasks_by_goal(&goal).await.unwrap();
        assert_eq!(tasks.len(), 3);
        assert!(tasks.iter().all(|task| task.status == TaskStatus::Pending));
        assert!(tasks.iter().all(|task| !task.path_claims.is_empty()));

        let stored = store.goal_plan(&goal).await.unwrap().unwrap();
        assert_eq!(stored, plan);
    }

    #[tokio::test]
    async fn only_tasks_with_satisfied_dependencies_are_ready() {
        let plan = Plan {
            tasks: vec![spec("a", &[]), spec("b", &["a"]), spec("c", &["b"])],
        };
        let (store, goal) = fixture(plan).await;

        let ready = store.ready_tasks(&goal).await.unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].title, "task a");

        store
            .set_task_status(&ready[0].id, TaskStatus::Running)
            .await
            .unwrap();
        assert!(store.ready_tasks(&goal).await.unwrap().is_empty());

        store
            .set_task_status(&ready[0].id, TaskStatus::Verifying)
            .await
            .unwrap();
        store
            .set_task_status(&ready[0].id, TaskStatus::Succeeded)
            .await
            .unwrap();

        let ready = store.ready_tasks(&goal).await.unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].title, "task b");
    }

    #[tokio::test]
    async fn a_failed_dependency_blocks_its_dependents_forever() {
        let plan = Plan {
            tasks: vec![spec("a", &[]), spec("b", &["a"])],
        };
        let (store, goal) = fixture(plan).await;

        let ready = store.ready_tasks(&goal).await.unwrap();
        store
            .set_task_status(&ready[0].id, TaskStatus::Running)
            .await
            .unwrap();
        store
            .set_task_status(&ready[0].id, TaskStatus::Failed)
            .await
            .unwrap();

        // "b" must not run on top of a failed dependency.
        assert!(store.ready_tasks(&goal).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn counts_describe_whether_a_goal_settled() {
        let plan = Plan {
            tasks: vec![spec("a", &[]), spec("b", &["a"])],
        };
        let (store, goal) = fixture(plan).await;

        let counts = store.goal_task_counts(&goal).await.unwrap();
        assert_eq!(counts.total, 2);
        assert_eq!(counts.pending, 2);
        assert!(!counts.settled());

        let ready = store.ready_tasks(&goal).await.unwrap();
        store
            .set_task_status(&ready[0].id, TaskStatus::Running)
            .await
            .unwrap();
        assert_eq!(store.goal_task_counts(&goal).await.unwrap().in_flight(), 1);
    }

    #[tokio::test]
    async fn an_invalid_plan_is_refused_before_any_row_is_written() {
        let store = Store::in_memory().await.unwrap();
        let project = jcs_core::Project {
            id: ProjectId::new(),
            name: "demo".into(),
            root_path: std::path::PathBuf::from("/tmp/demo"),
            base_branch: "main".into(),
            created_at: Timestamp::now(),
            auto_merge: false,
            budget_usd_per_day: None,
            budget_tokens_per_day: None,
        };
        store.insert_project(&project).await.unwrap();
        let goal = store.create_goal(&project.id, "objective").await.unwrap();

        let bad = Plan {
            tasks: vec![spec("a", &["missing"])],
        };
        assert!(store
            .create_planned_tasks(&goal.id, &bad, "{}")
            .await
            .is_err());
        assert!(store.list_tasks_by_goal(&goal.id).await.unwrap().is_empty());
    }
}
