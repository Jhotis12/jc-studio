use std::path::PathBuf;

use sqlx::FromRow;

use crate::error::{Result, StoreError};
use crate::store::Store;
use jcs_core::{EventKind, NewEvent, Project, ProjectId};

#[derive(Debug, FromRow)]
struct ProjectRow {
    id: String,
    name: String,
    root_path: String,
    base_branch: String,
    created_at: String,
    auto_merge: i64,
    budget_usd_per_day: Option<f64>,
    budget_tokens_per_day: Option<i64>,
}

impl ProjectRow {
    fn into_domain(self) -> Result<Project> {
        Ok(Project {
            id: ProjectId::parse(self.id)?,
            name: self.name,
            root_path: PathBuf::from(self.root_path),
            base_branch: self.base_branch,
            created_at: crate::store::decode_ts(&self.created_at)?,
            auto_merge: self.auto_merge != 0,
            budget_usd_per_day: self.budget_usd_per_day,
            budget_tokens_per_day: self.budget_tokens_per_day,
        })
    }
}

impl Store {
    /// Registers a project. Fails on a duplicate name or root path.
    pub async fn insert_project(&self, project: &Project) -> Result<()> {
        let mut tx = self.pool().begin().await?;

        let inserted = sqlx::query(
            "INSERT INTO projects \
             (id, name, root_path, base_branch, created_at, auto_merge, \
              budget_usd_per_day, budget_tokens_per_day) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(project.id.as_str())
        .bind(&project.name)
        .bind(project.root_path.to_string_lossy().to_string())
        .bind(&project.base_branch)
        .bind(crate::store::encode_ts(project.created_at))
        .bind(i64::from(project.auto_merge))
        .bind(project.budget_usd_per_day)
        .bind(project.budget_tokens_per_day)
        .execute(&mut *tx)
        .await;

        match inserted {
            Ok(_) => {}
            Err(sqlx::Error::Database(db)) if crate::store::is_unique_violation(db.as_ref()) => {
                return Err(StoreError::Conflict(format!(
                    "a project with name {:?} or path {:?} already exists",
                    project.name, project.root_path
                )));
            }
            Err(other) => return Err(other.into()),
        }

        let event = NewEvent::new(EventKind::ProjectAdded)
            .project(&project.id)
            .payload(serde_json::json!({
                "name": project.name,
                "root_path": project.root_path.to_string_lossy(),
                "base_branch": project.base_branch,
            }));
        crate::store::insert_event(&mut tx, &event).await?;

        tx.commit().await?;
        Ok(())
    }

    pub async fn list_projects(&self) -> Result<Vec<Project>> {
        let rows: Vec<ProjectRow> = sqlx::query_as(
            "SELECT id, name, root_path, base_branch, created_at, auto_merge, \
                    budget_usd_per_day, budget_tokens_per_day FROM projects ORDER BY name",
        )
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(ProjectRow::into_domain).collect()
    }

    pub async fn get_project(&self, id: &ProjectId) -> Result<Option<Project>> {
        let row: Option<ProjectRow> = sqlx::query_as(
            "SELECT id, name, root_path, base_branch, created_at, auto_merge, \
                    budget_usd_per_day, budget_tokens_per_day FROM projects WHERE id = ?",
        )
        .bind(id.as_str())
        .fetch_optional(self.pool())
        .await?;

        row.map(ProjectRow::into_domain).transpose()
    }

    /// Looks a project up by name, falling back to its root path.
    ///
    /// Clients accept either because "the repo I am standing in" and "the project I
    /// named" are both natural ways for a human to refer to a project.
    pub async fn resolve_project(&self, reference: &str) -> Result<Option<Project>> {
        if let Some(project) = self.find_project_by_name(reference).await? {
            return Ok(Some(project));
        }
        self.find_project_by_path(reference).await
    }

    pub async fn find_project_by_name(&self, name: &str) -> Result<Option<Project>> {
        let row: Option<ProjectRow> = sqlx::query_as(
            "SELECT id, name, root_path, base_branch, created_at, auto_merge, \
                    budget_usd_per_day, budget_tokens_per_day FROM projects WHERE name = ?",
        )
        .bind(name)
        .fetch_optional(self.pool())
        .await?;

        row.map(ProjectRow::into_domain).transpose()
    }

    pub async fn find_project_by_path(&self, path: &str) -> Result<Option<Project>> {
        let row: Option<ProjectRow> = sqlx::query_as(
            "SELECT id, name, root_path, base_branch, created_at, auto_merge, \
                    budget_usd_per_day, budget_tokens_per_day FROM projects WHERE root_path = ?",
        )
        .bind(path)
        .fetch_optional(self.pool())
        .await?;

        row.map(ProjectRow::into_domain).transpose()
    }

    /// Deletes a project and everything owned by it (tasks, runs, gate results).
    ///
    /// The event log is intentionally untouched: it is the audit record of what happened.
    pub async fn delete_project(&self, id: &ProjectId) -> Result<()> {
        let mut tx = self.pool().begin().await?;

        let result = sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id.as_str())
            .execute(&mut *tx)
            .await?;

        if result.rows_affected() == 0 {
            return Err(StoreError::not_found("project", id));
        }

        let event = NewEvent::new(EventKind::ProjectRemoved).project(id);
        crate::store::insert_event(&mut tx, &event).await?;

        tx.commit().await?;
        Ok(())
    }

    /// Replaces the project's execution settings (auto-merge and daily budgets).
    pub async fn update_project_settings(
        &self,
        id: &ProjectId,
        auto_merge: bool,
        budget_usd_per_day: Option<f64>,
        budget_tokens_per_day: Option<i64>,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE projects SET auto_merge = ?, budget_usd_per_day = ?, \
             budget_tokens_per_day = ? WHERE id = ?",
        )
        .bind(i64::from(auto_merge))
        .bind(budget_usd_per_day)
        .bind(budget_tokens_per_day)
        .bind(id.as_str())
        .execute(self.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(StoreError::not_found("project", id));
        }
        Ok(())
    }

    pub async fn project_count(&self) -> Result<i64> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
            .fetch_one(self.pool())
            .await?;
        Ok(count)
    }
}
