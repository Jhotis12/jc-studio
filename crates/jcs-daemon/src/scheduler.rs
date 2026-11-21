//! The scheduler: the only component that decides what runs next.
//!
//! It is a plain loop over durable state, so a restart loses nothing: readiness, leases,
//! budgets and goal status all live in SQLite. Every tick asks the database "what can run
//! now?" instead of trusting in-memory bookkeeping.
//!
//! Invariants it enforces:
//!   * a task runs only if its lease can be acquired (one owner, ever);
//!   * two tasks whose path claims overlap never run at the same time;
//!   * no work is dispatched for a project that exceeded its daily budget;
//!   * a goal's status is derived from its tasks, never written by a task.

use std::time::Duration;

use anyhow::Result;
use jcs_core::{EventKind, Goal, GoalStatus, NewEvent, Project, StatusMachine, Task};
use jcs_proto::PermissionMode;
use tokio::time::{interval, MissedTickBehavior};

use crate::executor::{self, TaskDispatch};
use crate::merge;
use crate::state::AppState;

const TICK: Duration = Duration::from_secs(1);
const DAY_MS: i64 = 86_400_000;

/// Starts the scheduler loop.
pub fn spawn(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Identifies this daemon instance as the lease owner.
        let owner = format!(
            "jcsd:{}:{}",
            std::process::id(),
            jiff::Timestamp::now().as_millisecond()
        );

        let mut ticker = interval(TICK);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;
            if let Err(error) = tick(&state, &owner).await {
                tracing::error!(error = %format!("{error:#}"), "scheduler tick failed");
            }
        }
    })
}

async fn tick(state: &AppState, owner: &str) -> Result<()> {
    let expired = state.store.expire_leases().await?;
    state.store.note_expired_leases(expired).await?;

    // Integrate before dispatching: a dependent task must be cut from an integration
    // branch that already contains its predecessors.
    merge::tick(state, owner).await?;

    for goal in state.store.active_goals().await? {
        let Some(project) = state.store.get_project(&goal.project_id).await? else {
            continue;
        };

        let counts = state.store.goal_task_counts(&goal.id).await?;

        // Planning in flight: nothing to schedule yet.
        if counts.total == 0 {
            continue;
        }

        if goal.status == GoalStatus::Pending {
            state
                .store
                .set_goal_status(&goal.id, GoalStatus::Running)
                .await?;
        }

        // A finished goal is finalized regardless of budget: the ceiling gates new work,
        // it does not retroactively block work that already completed.
        if counts.settled() {
            finalize_goal(state, &goal, &project, counts).await?;
            continue;
        }

        if counts.in_flight() > 0 {
            continue;
        }

        if let Some(reason) = budget_block(state, &project).await? {
            block_goal(state, &goal, &reason).await?;
            continue;
        }

        // Create the integration branch only when there is work to dispatch: branching a
        // blocked goal from a base that later moves leaves it permanently divergent.
        merge::ensure_integration_branch(state, &goal, &project, counts.total).await?;

        dispatch(state, owner, &goal, &project).await?;
    }

    Ok(())
}

async fn budget_block(state: &AppState, project: &Project) -> Result<Option<String>> {
    if project.budget_usd_per_day.is_none() && project.budget_tokens_per_day.is_none() {
        return Ok(None);
    }

    let now = jiff::Timestamp::now().as_millisecond();
    let start_of_day = now - now.rem_euclid(DAY_MS);
    let since =
        jiff::Timestamp::from_millisecond(start_of_day).unwrap_or_else(|_| jiff::Timestamp::now());

    let (cost, tokens) = state.store.usage_since(&project.id, since).await?;

    if let Some(limit) = project.budget_usd_per_day {
        if cost >= limit {
            return Ok(Some(format!(
                "daily budget reached for {}: ${cost:.4} of ${limit:.2}",
                project.name
            )));
        }
    }
    if let Some(limit) = project.budget_tokens_per_day {
        if tokens >= limit {
            return Ok(Some(format!(
                "daily token budget reached for {}: {tokens} of {limit}",
                project.name
            )));
        }
    }

    Ok(None)
}

async fn block_goal(state: &AppState, goal: &Goal, reason: &str) -> Result<()> {
    if goal.status == GoalStatus::Blocked {
        return Ok(());
    }

    if goal.status != GoalStatus::Running && goal.status != GoalStatus::Pending {
        return Ok(());
    }

    state
        .store
        .set_goal_status(&goal.id, GoalStatus::Blocked)
        .await?;

    let event = NewEvent::new(EventKind::Error)
        .project(&goal.project_id)
        .goal(&goal.id)
        .payload(serde_json::json!({
            "category": "budget_exceeded",
            "reason": reason,
            "hint": "raise the budget or wait for the daily window to reset, then resume the goal",
        }));
    jcs_store::append_event(&state.store, event).await?;

    tracing::warn!(goal = %goal.id, reason, "goal parked: budget exhausted");
    Ok(())
}

async fn finalize_goal(
    state: &AppState,
    goal: &Goal,
    project: &Project,
    counts: jcs_store::TaskCounts,
) -> Result<()> {
    let next = if counts.failed > 0 {
        GoalStatus::Failed
    } else if counts.blocked > 0 {
        GoalStatus::Blocked
    } else if counts.cancelled > 0 {
        GoalStatus::Cancelled
    } else if counts.succeeded == counts.total {
        // A goal is not finished until every succeeded task is on the integration branch.
        if state.store.goal_pending_merges(&goal.id).await? > 0 {
            return Ok(());
        }

        // Work can be perfect and still not land: a merge conflict is a goal-level
        // failure, not a task failure.
        if state.store.goal_has_merge_errors(&goal.id).await? {
            let event = NewEvent::new(EventKind::Error)
                .project(&project.id)
                .goal(&goal.id)
                .payload(serde_json::json!({
                    "category": "merge_failed",
                    "hint": "fix the conflict in the task worktree, then `jcs resume`",
                }));
            jcs_store::append_event(&state.store, event).await?;
            GoalStatus::Blocked
        } else if project.auto_merge {
            match state.store.goal_integration_branch(&goal.id).await? {
                Some(integration) => {
                    if let Err(error) = merge::promote(state, goal, project, &integration).await {
                        let event = NewEvent::new(EventKind::Error)
                            .project(&project.id)
                            .goal(&goal.id)
                            .payload(serde_json::json!({
                                "category": "promotion_failed",
                                "reason": format!("{error:#}"),
                                "hint": "integrate the branch manually, then `jcs resume`",
                            }));
                        jcs_store::append_event(&state.store, event).await?;
                        GoalStatus::Blocked
                    } else {
                        GoalStatus::Succeeded
                    }
                }
                None => GoalStatus::Succeeded,
            }
        } else {
            GoalStatus::Succeeded
        }
    } else {
        return Ok(());
    };

    if goal.status == next {
        return Ok(());
    }
    if !goal.status.can_transition_to(next) {
        tracing::warn!(
            goal = %goal.id,
            from = goal.status.as_str(),
            to = next.as_str(),
            "illegal goal transition; leaving the goal untouched"
        );
        return Ok(());
    }

    state.store.set_goal_status(&goal.id, next).await?;
    tracing::info!(goal = %goal.id, status = next.as_str(), "goal finished");
    Ok(())
}

async fn dispatch(state: &AppState, owner: &str, goal: &Goal, project: &Project) -> Result<()> {
    let ready = state.store.ready_tasks(&goal.id).await?;
    if ready.is_empty() {
        return Ok(());
    }

    let settings = state.store.goal_settings(&goal.id).await?;
    let permissions = match settings.permissions.as_str() {
        "strict" => PermissionMode::Strict,
        _ => PermissionMode::Auto,
    };

    // Claims of everything already running in this project, including other goals.
    let mut claimed: Vec<Vec<String>> = state
        .store
        .in_flight_tasks(&project.id)
        .await?
        .into_iter()
        .map(|task| task.path_claims)
        .collect();

    let ttl = Duration::from_secs(state.config.lease_ttl_secs);

    for task in ready {
        if claims_conflict(&task.path_claims, &claimed) {
            tracing::debug!(
                task = %task.id,
                claims = ?task.path_claims,
                "skipping task: path claims overlap with a running task"
            );
            continue;
        }

        // Capacity before ownership: holding a lease while waiting for a permit would
        // leave the task "dispatching" forever from the budget's point of view.
        let permit = match state.permits.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => break,
        };

        let resource = lease_resource(&task);
        if !state
            .store
            .acquire_lease(&resource, &task.id, owner, ttl)
            .await?
        {
            drop(permit);
            continue;
        }

        tracing::info!(
            task = %task.id,
            goal = %goal.id,
            title = %task.title,
            "dispatching task"
        );

        executor::spawn(
            state.clone(),
            TaskDispatch {
                task_id: task.id.clone(),
                model: settings.model.clone(),
                agent: settings.agent.clone(),
                permissions,
                lease_owner: owner.to_string(),
            },
            permit,
        );

        claimed.push(task.path_claims.clone());
    }

    Ok(())
}

fn lease_resource(task: &Task) -> String {
    format!("task:{}", task.id)
}

/// Whether two sets of path claims could touch the same files.
///
/// Claims are directory prefixes, so overlap is a prefix relationship on segment
/// boundaries: `src/api` and `src/api/users` overlap; `src/a` and `src/ab` do not.
pub fn claims_conflict(a: &[String], b: &[Vec<String>]) -> bool {
    // Nothing running: nothing to collide with.
    if b.is_empty() {
        return false;
    }

    // A task that declares no claims says nothing about what it touches. Running it next
    // to anything else would be a guess, so it waits for a clear field.
    if a.is_empty() || b.iter().any(|claims| claims.is_empty()) {
        return true;
    }

    a.iter().any(|left| {
        b.iter()
            .any(|claims| claims.iter().any(|right| paths_overlap(left, right)))
    })
}

fn paths_overlap(left: &str, right: &str) -> bool {
    let left = left.trim_end_matches('/');
    let right = right.trim_end_matches('/');

    if left.is_empty() || right.is_empty() {
        // An empty claim says nothing about what the task touches; treat it as
        // conflicting with everything so it cannot silently race another task.
        return true;
    }

    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|rest| rest.starts_with('/'))
        || right
            .strip_prefix(left)
            .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_and_nested_claims_conflict() {
        assert!(paths_overlap("src/api", "src/api"));
        assert!(paths_overlap("src/api", "src/api/users"));
        assert!(paths_overlap("src/api/users", "src/api"));
    }

    #[test]
    fn sibling_prefixes_do_not_conflict() {
        assert!(!paths_overlap("src/a", "src/ab"));
        assert!(!paths_overlap("src/api", "src/db"));
        assert!(!paths_overlap("docs", "src"));
    }

    #[test]
    fn trailing_slashes_are_ignored() {
        assert!(paths_overlap("src/api/", "src/api"));
        assert!(!paths_overlap("src/a/", "src/ab"));
    }

    #[test]
    fn an_empty_claim_conflicts_with_everything_that_is_running() {
        assert!(paths_overlap("", "src/api"));
        assert!(conflict_with(&[], "src/api"));
        // With nothing in flight, an empty claim is harmless.
        assert!(!claims_conflict(&[], &[]));
        assert!(!claims_conflict(&["src/api".into()], &[]));
    }

    fn conflict_with(claims: &[&str], other: &str) -> bool {
        let a: Vec<String> = claims.iter().map(|value| value.to_string()).collect();
        let b = vec![vec![other.to_string()]];
        claims_conflict(&a, &b)
    }

    #[test]
    fn claim_conflicts_are_detected_across_tasks() {
        let running = vec![vec!["src/api".to_string()], vec!["docs".to_string()]];
        assert!(claims_conflict(&["src/api/users".into()], &running));
        assert!(!claims_conflict(&["src/db".into()], &running));
    }
}
