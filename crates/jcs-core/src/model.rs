use std::path::PathBuf;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::ids::{GoalId, ProjectId, RunId, TaskId};
use crate::plan::TaskRole;
use crate::status::{GoalStatus, RunStatus, TaskStatus};

/// A registered git repository that JC Studio can work on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub root_path: PathBuf,
    /// Branch new task worktrees are cut from.
    pub base_branch: String,
    pub created_at: Timestamp,
    /// When true, succeeded tasks are merged into `base_branch` by the merge queue.
    pub auto_merge: bool,
    /// Daily spend ceiling. `None` means unbounded.
    pub budget_usd_per_day: Option<f64>,
    /// Daily token ceiling. `None` means unbounded.
    pub budget_tokens_per_day: Option<i64>,
}

/// A user objective, decomposed (eventually) into tasks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    pub id: GoalId,
    pub project_id: ProjectId,
    pub prompt: String,
    pub status: GoalStatus,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// A unit of work that one agent session is asked to complete.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub goal_id: GoalId,
    pub title: String,
    /// Standalone instructions for the implementer. Set from the plan or from operator
    /// feedback when a task is iterated.
    pub instructions: Option<String>,
    pub status: TaskStatus,
    /// Absolute path of the isolated worktree, once created.
    pub worktree: Option<PathBuf>,
    /// Git branch checked out in the worktree.
    pub branch: Option<String>,
    /// OpenCode session bound to this task.
    pub session_id: Option<String>,
    /// What the task is for; shaped into the prompt and used in diagnostics.
    pub role: TaskRole,
    /// Higher runs first among ready tasks.
    pub priority: i32,
    /// Repository-relative paths this task expects to touch. The scheduler refuses to run
    /// two tasks whose claims overlap.
    pub path_claims: Vec<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// One execution attempt of a task.
///
/// Attempts are immutable once finished: a retry creates a new run with `attempt + 1`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub task_id: TaskId,
    pub attempt: u32,
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    /// Cumulative cost reported by the agent backend, in US dollars.
    pub cost_usd: f64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub error: Option<String>,
}

impl Run {
    /// Duration of a finished run in milliseconds, or `None` while it is still running.
    pub fn duration_millis(&self) -> Option<i64> {
        self.ended_at
            .map(|end| end.as_millisecond() - self.started_at.as_millisecond())
    }

    /// Whether the run ended in a state that allows an automatic retry.
    ///
    /// `Interrupted` qualifies: the process died, which proves nothing about the task.
    pub fn is_retryable(&self) -> bool {
        matches!(self.status, RunStatus::Interrupted | RunStatus::TimedOut)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_run(status: RunStatus) -> Run {
        Run {
            id: RunId::new(),
            task_id: TaskId::new(),
            attempt: 1,
            status,
            exit_code: None,
            started_at: Timestamp::now(),
            ended_at: None,
            cost_usd: 0.0,
            tokens_in: 0,
            tokens_out: 0,
            error: None,
        }
    }

    #[test]
    fn only_process_failures_are_retryable() {
        assert!(sample_run(RunStatus::Interrupted).is_retryable());
        assert!(sample_run(RunStatus::TimedOut).is_retryable());
        assert!(!sample_run(RunStatus::Failed).is_retryable());
        assert!(!sample_run(RunStatus::Succeeded).is_retryable());
    }

    #[test]
    fn duration_requires_an_end() {
        let running = sample_run(RunStatus::Started);
        assert!(running.duration_millis().is_none());
    }
}
