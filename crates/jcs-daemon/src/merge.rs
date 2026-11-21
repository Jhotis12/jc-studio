//! Integration branches and the merge queue.
//!
//! Two different merges live here, and conflating them would be a bug:
//!
//!   * **task -> integration**: always required. It is what makes `depends_on` real, because
//!     the next task's worktree is cut from the integration branch.
//!   * **integration -> base**: optional (`auto_merge`). It touches the user's checkout, so
//!     it only happens when the whole goal succeeded and the tree is clean.
//!
//! Merges are serialized per project by a lease: two rebases into the same integration
//! branch at once would corrupt it.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use jcs_core::{EventKind, Goal, NewEvent, Project, Task};
use jcs_git::Repo;

use crate::state::AppState;

const MERGE_LEASE_TTL: Duration = Duration::from_secs(600);

/// Creates the goal's integration branch the first time it is needed.
pub async fn ensure_integration_branch(
    state: &AppState,
    goal: &Goal,
    project: &Project,
    task_count: i64,
) -> Result<Option<String>> {
    // A single task with auto-merge off keeps the old, simpler behaviour: its branch is
    // the deliverable and nothing needs an integration branch.
    if task_count <= 1 && !project.auto_merge {
        return Ok(None);
    }

    if let Some(branch) = state.store.goal_integration_branch(&goal.id).await? {
        return Ok(Some(branch));
    }

    let branch = format!("jcs/goal/{}", suffix(goal.id.as_str()));
    let repo = Repo::new(&project.root_path);

    if !repo.branch_exists(&branch).await? {
        repo.create_branch(&branch, &project.base_branch)
            .await
            .with_context(|| {
                format!(
                    "cannot create the integration branch {branch} from {}",
                    project.base_branch
                )
            })?;
    }

    state
        .store
        .set_goal_integration_branch(&goal.id, &branch)
        .await?;

    let event = NewEvent::new(EventKind::MergeFinished)
        .project(&project.id)
        .goal(&goal.id)
        .payload(serde_json::json!({
            "result": "integration_branch_created",
            "branch": branch,
            "from": project.base_branch,
        }));
    jcs_store::append_event(&state.store, event).await?;

    Ok(Some(branch))
}

/// Processes at most one pending merge per project per tick.
pub async fn tick(state: &AppState, owner: &str) -> Result<()> {
    let pending = state.store.tasks_pending_merge().await?;

    for task in pending {
        let Some(goal) = state.store.get_goal(&task.goal_id).await? else {
            continue;
        };
        let Some(project) = state.store.get_project(&goal.project_id).await? else {
            continue;
        };
        let Some(integration) = state.store.goal_integration_branch(&goal.id).await? else {
            continue;
        };

        let resource = format!("merge:{}", project.id);
        if !state
            .store
            .acquire_lease(&resource, &task.id, owner, MERGE_LEASE_TTL)
            .await?
        {
            // Another merge is in progress for this project; try later.
            continue;
        }

        let result = merge_task(state, &project, &task, &integration).await;
        if let Err(error) = state.store.release_lease(&resource, owner).await {
            tracing::debug!(error = %error, "cannot release the merge lease");
        }

        match result {
            Ok(()) => {
                tracing::info!(task = %task.id, integration = %integration, "task merged");
            }
            Err(error) => {
                let message = format!("{error:#}");
                tracing::error!(task = %task.id, error = %message, "merge failed");
                state
                    .store
                    .mark_task_merge_failed(&task.id, &message)
                    .await?;
            }
        }

        // One merge per project per tick keeps the history linear and the output readable.
        return Ok(());
    }

    Ok(())
}

/// Rebases the task onto the integration branch, re-verifies, then fast-forwards.
async fn merge_task(
    state: &AppState,
    project: &Project,
    task: &Task,
    integration: &str,
) -> Result<()> {
    let worktree = task
        .worktree
        .as_ref()
        .ok_or_else(|| anyhow!("task {} has no worktree to merge from", task.id))?;
    let branch = task
        .branch
        .as_ref()
        .ok_or_else(|| anyhow!("task {} has no branch to merge", task.id))?;

    let repo = Repo::new(&project.root_path);
    let worktree_repo = Repo::new(worktree);

    // Already on top of the integration branch: nothing to replay.
    if !repo.is_ancestor(integration, branch).await? {
        if let Err(error) = worktree_repo.rebase_onto(integration).await {
            let _ = worktree_repo.rebase_abort().await;
            return Err(anyhow!(
                "cannot rebase {branch} onto {integration}: {error}"
            ));
        }

        // The code changed under the task, so the previous verdict is void.
        let evaluation = crate::verify::evaluate(state, project, worktree).await?;
        if !evaluation.passed() {
            return Err(anyhow!(
                "gates failed after rebasing onto {integration}: {}",
                evaluation.failed_names().join(", ")
            ));
        }
    }

    let integration_worktree = integration_worktree(state, project, integration).await?;
    integration_worktree
        .merge_ff_only(branch)
        .await
        .with_context(|| format!("cannot fast-forward {integration} to {branch}"))?;

    let sha = integration_worktree.short_head().await?;
    state.store.mark_task_merged(&task.id, &sha).await?;

    // The commit now lives in the integration branch; the task worktree is disposable.
    if let Err(error) = repo.remove_worktree(worktree, true).await {
        tracing::debug!(error = %error, "cannot remove the task worktree after merging");
    }
    if let Err(error) = repo.delete_branch(branch, true).await {
        tracing::debug!(error = %error, "cannot delete the merged task branch");
    }

    Ok(())
}

async fn integration_worktree(
    state: &AppState,
    project: &Project,
    integration: &str,
) -> Result<Repo> {
    let path = integration_worktree_path(state, project, integration);

    if path.exists() {
        return Ok(Repo::new(path));
    }

    let repo = Repo::new(&project.root_path);
    repo.add_worktree(&path, integration)
        .await
        .with_context(|| {
            format!(
                "cannot check out the integration branch {integration} at {}",
                path.display()
            )
        })?;

    Ok(Repo::new(path))
}

fn integration_worktree_path(state: &AppState, project: &Project, integration: &str) -> PathBuf {
    state
        .config
        .worktrees_dir
        .join(project.id.as_str())
        .join(format!("integration-{}", integration.replace('/', "-")))
}

/// Promotes the integration branch into the base branch.
///
/// Fast-forward only, and only from a clean tree: this writes to the user's checkout, so it
/// refuses rather than resolving anything on the operator's behalf.
pub async fn promote(
    state: &AppState,
    goal: &Goal,
    project: &Project,
    integration: &str,
) -> Result<()> {
    let repo = Repo::new(&project.root_path);

    if repo.is_ancestor(integration, &project.base_branch).await? {
        // Already promoted (a retry after a crash mid-way).
        cleanup(state, project, integration);
        return Ok(());
    }

    if !repo.is_clean().await? {
        return Err(anyhow!(
            "{} has uncommitted changes; refusing to touch the base branch",
            project.root_path.display()
        ));
    }

    // The base branch must be the one checked out for this to be a fast-forward.
    let current = repo.current_branch().await?;
    if current != project.base_branch {
        return Err(anyhow!(
            "{} has {current} checked out, not {}",
            project.root_path.display(),
            project.base_branch
        ));
    }

    repo.merge_ff_only(integration).await.with_context(|| {
        format!(
            "cannot fast-forward {} to {integration}",
            project.base_branch
        )
    })?;

    let event = NewEvent::new(EventKind::MergeFinished)
        .project(&project.id)
        .goal(&goal.id)
        .payload(serde_json::json!({
            "result": "promoted",
            "branch": integration,
            "into": project.base_branch,
        }));
    jcs_store::append_event(&state.store, event).await?;

    cleanup(state, project, integration);
    Ok(())
}

fn cleanup(state: &AppState, project: &Project, integration: &str) {
    let path = integration_worktree_path(state, project, integration);
    let repo = Repo::new(&project.root_path);

    if !path.exists() {
        return;
    }

    // Blocking is not an option here; spawn a best-effort cleanup.
    let store_repo = repo.clone();
    let branch = integration.to_string();
    tokio::spawn(async move {
        if let Err(error) = store_repo.remove_worktree(&path, true).await {
            tracing::debug!(error = %error, "cannot remove the integration worktree");
            return;
        }
        if let Err(error) = store_repo.delete_branch(&branch, true).await {
            tracing::debug!(error = %error, "cannot delete the integration branch");
        }
    });
}

/// Last eight alphanumeric characters of an id, for human-readable branch names.
fn suffix(id: &str) -> String {
    let body: String = id.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let start = body.len().saturating_sub(8);
    body[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffixes_are_stable_and_bounded() {
        assert_eq!(suffix("goal_0123456789abcdef"), "89abcdef");
        assert_eq!(suffix("goal_abc"), "goalabc");
        assert!(suffix("goal_abc").chars().count() <= 8);
    }
}
