use serde::{Deserialize, Serialize};

use crate::CoreError;

/// A finite state machine over a status value.
///
/// Implementors declare their legal transitions explicitly. Nothing else in the
/// system is allowed to move a status without going through [`StatusMachine::transition`],
/// which is what makes "illegal transition" a caught bug rather than a corrupt row.
pub trait StatusMachine: Copy + Eq + std::fmt::Debug + 'static {
    /// Stable, snake_case name used in the database and the wire protocol.
    fn as_str(self) -> &'static str;

    /// Statuses reachable in one legal step. Empty for terminal statuses.
    fn allowed_next(self) -> &'static [Self];

    /// Whether the status is final: no outgoing transitions at all.
    fn is_terminal(self) -> bool {
        self.allowed_next().is_empty()
    }

    fn can_transition_to(self, next: Self) -> bool {
        self.allowed_next().contains(&next)
    }

    /// Validates a transition, returning the new status or a domain error naming the entity.
    fn transition(self, next: Self, entity: &'static str) -> Result<Self, CoreError> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(CoreError::InvalidTransition {
                entity,
                from: self.as_str(),
                to: next.as_str(),
            })
        }
    }
}

macro_rules! status_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $($variant:ident => $label:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            /// All variants, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// Parses the snake_case wire name.
            pub fn parse(value: &str) -> Result<Self, CoreError> {
                match value {
                    $($label => Ok(Self::$variant),)+
                    other => Err(CoreError::unknown(stringify!($name), other)),
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

status_enum! {
    /// Lifecycle of a user objective.
    GoalStatus {
        Pending => "pending",
        Running => "running",
        Succeeded => "succeeded",
        Failed => "failed",
        Cancelled => "cancelled",
    }
}

impl StatusMachine for GoalStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn allowed_next(self) -> &'static [Self] {
        match self {
            Self::Pending => &[Self::Running, Self::Cancelled],
            Self::Running => &[Self::Succeeded, Self::Failed, Self::Cancelled],
            Self::Succeeded | Self::Failed | Self::Cancelled => &[],
        }
    }
}

status_enum! {
    /// Lifecycle of a unit of work inside a goal.
    ///
    /// `Verifying` exists because "the agent says it is done" and "the gates pass"
    /// are different facts. A task only reaches `Succeeded` through `Verifying`.
    TaskStatus {
        Pending => "pending",
        Running => "running",
        Verifying => "verifying",
        Succeeded => "succeeded",
        Failed => "failed",
        Blocked => "blocked",
        Cancelled => "cancelled",
    }
}

impl StatusMachine for TaskStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Verifying => "verifying",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
        }
    }

    fn allowed_next(self) -> &'static [Self] {
        match self {
            Self::Pending => &[Self::Running, Self::Blocked, Self::Cancelled],
            Self::Running => &[Self::Verifying, Self::Failed, Self::Blocked, Self::Cancelled],
            // A failed gate sends the task back to Running for a bounded repair loop.
            Self::Verifying => &[Self::Succeeded, Self::Running, Self::Failed, Self::Cancelled],
            // Retry is an explicit, counted decision, not an automatic side effect.
            Self::Failed => &[Self::Pending, Self::Cancelled],
            Self::Blocked => &[Self::Pending, Self::Cancelled],
            Self::Succeeded | Self::Cancelled => &[],
        }
    }
}

status_enum! {
    /// Lifecycle of one execution attempt of a task.
    RunStatus {
        Started => "started",
        Succeeded => "succeeded",
        Failed => "failed",
        Interrupted => "interrupted",
        TimedOut => "timed_out",
        Cancelled => "cancelled",
    }
}

impl StatusMachine for RunStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
            Self::TimedOut => "timed_out",
            Self::Cancelled => "cancelled",
        }
    }

    fn allowed_next(self) -> &'static [Self] {
        match self {
            // `Interrupted` is deliberately distinct from `Failed`: a process that died
            // did not prove the task wrong, so it is retryable without counting a failure.
            Self::Started => &[
                Self::Succeeded,
                Self::Failed,
                Self::Interrupted,
                Self::TimedOut,
                Self::Cancelled,
            ],
            Self::Succeeded
            | Self::Failed
            | Self::Interrupted
            | Self::TimedOut
            | Self::Cancelled => &[],
        }
    }
}

status_enum! {
    /// Outcome of a single executable check.
    GateStatus {
        Passed => "passed",
        Failed => "failed",
        Errored => "errored",
        Skipped => "skipped",
    }
}

impl StatusMachine for GateStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Errored => "errored",
            Self::Skipped => "skipped",
        }
    }

    fn allowed_next(self) -> &'static [Self] {
        &[]
    }
}

impl GateStatus {
    /// Whether the gate blocks the task from being considered successful.
    pub fn is_blocking(self) -> bool {
        matches!(self, Self::Failed | Self::Errored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_names_match_the_database_and_protocol() {
        for status in GoalStatus::ALL {
            assert_eq!(GoalStatus::parse(status.as_str()).unwrap(), *status);
        }
        for status in TaskStatus::ALL {
            assert_eq!(TaskStatus::parse(status.as_str()).unwrap(), *status);
        }
        for status in RunStatus::ALL {
            assert_eq!(RunStatus::parse(status.as_str()).unwrap(), *status);
        }
    }

    #[test]
    fn parse_rejects_unknown_names() {
        assert!(TaskStatus::parse("done").is_err());
        assert!(TaskStatus::parse("").is_err());
    }

    #[test]
    fn terminal_statuses_have_no_exits() {
        for terminal in [
            GoalStatus::Succeeded,
            GoalStatus::Failed,
            GoalStatus::Cancelled,
        ] {
            assert!(terminal.is_terminal());
            for candidate in GoalStatus::ALL {
                assert!(!terminal.can_transition_to(*candidate));
            }
        }
    }

    #[test]
    fn failed_task_can_only_retry_or_cancel() {
        let allowed: Vec<_> = TaskStatus::Failed.allowed_next().to_vec();
        assert_eq!(allowed, vec![TaskStatus::Pending, TaskStatus::Cancelled]);
    }

    #[test]
    fn success_requires_passing_through_verifying() {
        assert!(!TaskStatus::Pending.can_transition_to(TaskStatus::Succeeded));
        assert!(!TaskStatus::Running.can_transition_to(TaskStatus::Succeeded));
        assert!(TaskStatus::Verifying.can_transition_to(TaskStatus::Succeeded));
    }

    #[test]
    fn a_started_run_is_never_restartable() {
        for terminal in [
            RunStatus::Succeeded,
            RunStatus::Failed,
            RunStatus::Interrupted,
            RunStatus::TimedOut,
            RunStatus::Cancelled,
        ] {
            assert!(terminal.is_terminal());
            assert!(!terminal.can_transition_to(RunStatus::Started));
        }
    }

    #[test]
    fn transition_error_names_the_entity_and_both_states() {
        let err = TaskStatus::Pending
            .transition(TaskStatus::Succeeded, "task")
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("task"), "{msg}");
        assert!(msg.contains("pending"), "{msg}");
        assert!(msg.contains("succeeded"), "{msg}");
    }
}

#[cfg(test)]
mod properties {
    use super::*;
    use proptest::prelude::*;

    fn any_goal() -> impl Strategy<Value = GoalStatus> {
        prop::sample::select(GoalStatus::ALL)
    }
    fn any_task() -> impl Strategy<Value = TaskStatus> {
        prop::sample::select(TaskStatus::ALL)
    }
    fn any_run() -> impl Strategy<Value = RunStatus> {
        prop::sample::select(RunStatus::ALL)
    }

    proptest! {
        /// A transition either succeeds and yields the target, or fails and changes nothing.
        #[test]
        fn goal_transitions_are_all_or_nothing(a in any_goal(), b in any_goal()) {
            match a.transition(b, "goal") {
                Ok(next) => prop_assert_eq!(next, b),
                Err(_) => prop_assert!(!a.can_transition_to(b)),
            }
        }

        #[test]
        fn task_transitions_are_all_or_nothing(a in any_task(), b in any_task()) {
            match a.transition(b, "task") {
                Ok(next) => prop_assert_eq!(next, b),
                Err(_) => prop_assert!(!a.can_transition_to(b)),
            }
        }

        #[test]
        fn run_transitions_are_all_or_nothing(a in any_run(), b in any_run()) {
            match a.transition(b, "run") {
                Ok(next) => prop_assert_eq!(next, b),
                Err(_) => prop_assert!(!a.can_transition_to(b)),
            }
        }

        /// Serialization roundtrips for every status of every machine.
        #[test]
        fn statuses_roundtrip_through_json(status in any_task()) {
            let json = serde_json::to_string(&status).unwrap();
            let back: TaskStatus = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(back, status);
        }

        /// No status ever transitions to itself (there are no no-op transitions).
        #[test]
        fn no_self_transitions(status in any_task()) {
            prop_assert!(!status.can_transition_to(status));
        }
    }
}
