use sqlx::FromRow;

use crate::error::{Result, StoreError};
use crate::store::Store;
use jcs_core::{EventKind, Goal, GoalId, GoalStatus, NewEvent, ProjectId, StatusMachine};

#[derive(Debug, FromRow)]
struct GoalRow {
    id: String,
    project_id: String,
    prompt: String,
    status: String,
    created_at: String,
    updated_at: String,
}

impl GoalRow {
    fn into_domain(self) -> Result<Goal> {
        Ok(Goal {
            id: GoalId::parse(self.id)?,
            project_id: ProjectId::parse(self.project_id)?,
            prompt: self.prompt,
            status: GoalStatus::parse(&self.status)?,
            created_at: crate::store::decode_ts(&self.created_at)?,
            updated_at: crate::store::decode_ts(&self.updated_at)?,
        })
    }
}

const GOAL_COLUMNS: &str = "id, project_id, prompt, status, created_at, updated_at";

impl Store {
    pub async fn create_goal(&self, project_id: &ProjectId, prompt: &str) -> Result<Goal> {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Err(StoreError::Domain(jcs_core::CoreError::EmptyField {
                field: "prompt",
            }));
        }

        // Fail early with a clear message instead of a foreign-key violation.
        if self.get_project(project_id).await?.is_none() {
            return Err(StoreError::not_found("project", project_id));
        }

        let now = chrono::Utc::now();
        let goal = Goal {
            id: GoalId::new(),
            project_id: project_id.clone(),
            prompt: prompt.to_string(),
            status: GoalStatus::Pending,
            created_at: now,
            updated_at: now,
        };

        let mut tx = self.pool().begin().await?;
        sqlx::query(
            "INSERT INTO goals (id, project_id, prompt, status, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(goal.id.as_str())
        .bind(goal.project_id.as_str())
        .bind(&goal.prompt)
        .bind(goal.status.as_str())
        .bind(crate::store::encode_ts(goal.created_at))
        .bind(crate::store::encode_ts(goal.updated_at))
        .execute(&mut *tx)
        .await?;

        let event = NewEvent::new(EventKind::GoalCreated)
            .project(project_id)
            .goal(&goal.id)
            .payload(serde_json::json!({ "prompt": goal.prompt, "status": goal.status.as_str() }));
        crate::store::insert_event(&mut tx, &event).await?;

        tx.commit().await?;
        Ok(goal)
    }

    /// Moves a goal to `next`, rejecting illegal transitions inside the transaction.
    pub async fn set_goal_status(&self, id: &GoalId, next: GoalStatus) -> Result<Goal> {
        let mut tx = self.pool().begin().await?;

        let current: Option<String> = sqlx::query_scalar("SELECT status FROM goals WHERE id = ?")
            .bind(id.as_str())
            .fetch_optional(&mut *tx)
            .await?;
        let current = current.ok_or_else(|| StoreError::not_found("goal", id))?;
        let current = GoalStatus::parse(&current)?;
        current.transition(next, "goal")?;

        sqlx::query("UPDATE goals SET status = ?, updated_at = ? WHERE id = ?")
            .bind(next.as_str())
            .bind(crate::store::encode_ts(chrono::Utc::now()))
            .bind(id.as_str())
            .execute(&mut *tx)
            .await?;

        let event = NewEvent::new(EventKind::GoalStatusChanged)
            .goal(id)
            .payload(serde_json::json!({ "from": current.as_str(), "to": next.as_str() }));
        crate::store::insert_event(&mut tx, &event).await?;

        tx.commit().await?;
        self.get_goal(id)
            .await?
            .ok_or_else(|| StoreError::not_found("goal", id))
    }

    pub async fn get_goal(&self, id: &GoalId) -> Result<Option<Goal>> {
        let row: Option<GoalRow> =
            sqlx::query_as(&format!("SELECT {GOAL_COLUMNS} FROM goals WHERE id = ?"))
                .bind(id.as_str())
                .fetch_optional(self.pool())
                .await?;

        row.map(GoalRow::into_domain).transpose()
    }

    pub async fn list_goals(&self, limit: i64) -> Result<Vec<Goal>> {
        let rows: Vec<GoalRow> = sqlx::query_as(&format!(
            "SELECT {GOAL_COLUMNS} FROM goals ORDER BY created_at DESC LIMIT ?"
        ))
        .bind(limit.clamp(1, 1_000))
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(GoalRow::into_domain).collect()
    }

    /// Total number of goals, independent of any list limit.
    pub async fn goal_count(&self) -> Result<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM goals")
            .fetch_one(self.pool())
            .await?;
        Ok(count)
    }

    pub async fn list_goals_by_project(
        &self,
        project_id: &ProjectId,
        limit: i64,
    ) -> Result<Vec<Goal>> {
        let rows: Vec<GoalRow> = sqlx::query_as(&format!(
            "SELECT {GOAL_COLUMNS} FROM goals WHERE project_id = ? ORDER BY created_at DESC LIMIT ?"
        ))
        .bind(project_id.as_str())
        .bind(limit.clamp(1, 1_000))
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(GoalRow::into_domain).collect()
    }
}
