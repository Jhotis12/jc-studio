use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{GoalId, ProjectId, RunId, TaskId};
use crate::status::{GoalStatus, RunStatus, TaskStatus};

/// A registered git repository that JC Studio can work on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub root_path: PathBuf,
    /// Branch new task worktrees are cut from.
    pub base_branch: String,
    pub created_at: DateTime<Utc>,
}

/// A user objective, decomposed (eventually) into tasks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Goal {
    pub id: GoalId,
    pub project_id: ProjectId,
    pub prompt: String,
    pub status: GoalStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A unit of work that one agent session is asked to complete.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub goal_id: GoalId,
    pub title: String,
    pub status: TaskStatus,
    /// Absolute path of the isolated worktree, once created.
    pub worktree: Option<PathBuf>,
    /// Git branch checked out in the worktree.
    pub branch: Option<String>,
    /// OpenCode session bound to this task.
    pub session_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    /// Cumulative cost reported by the agent backend, in US dollars.
    pub cost_usd: f64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub error: Option<String>,
}

impl Run {
    /// Duration of a finished run, or `None` while it is still running.
    pub fn duration(&self) -> Option<chrono::Duration> {
        self.ended_at.map(|end| end - self.started_at)
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
            started_at: Utc::now(),
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
        assert!(running.duration().is_none());
    }
}
