//! Plans: the decomposition of a goal into a dependency-ordered set of tasks.
//!
//! A planner is a language model, so its output is untrusted input. Everything here is
//! about turning that input into something the scheduler can rely on: unique keys, real
//! dependencies, no cycles, relative paths, bounded size.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::CoreError;

/// Maximum number of tasks a plan may contain.
///
/// A plan is a unit of human review as much as of execution: beyond this, the objective
/// is too large for one shot and should be split by a human.
pub const MAX_PLAN_TASKS: usize = 12;

/// What a task is for. The scheduler uses it for prompt shaping and diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskRole {
    #[default]
    Implement,
    Test,
    Review,
    Docs,
    Migrate,
}

impl TaskRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Implement => "implement",
            Self::Test => "test",
            Self::Review => "review",
            Self::Docs => "docs",
            Self::Migrate => "migrate",
        }
    }

    pub fn parse(value: &str) -> Result<Self, CoreError> {
        match value {
            "implement" => Ok(Self::Implement),
            "test" => Ok(Self::Test),
            "review" => Ok(Self::Review),
            "docs" => Ok(Self::Docs),
            "migrate" => Ok(Self::Migrate),
            other => Err(CoreError::unknown("TaskRole", other)),
        }
    }
}

/// One task as the planner described it, before it becomes a database row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSpec {
    /// Planner-local key, referenced by `depends_on`.
    pub key: String,
    pub title: String,
    /// What to do, in enough detail to be a standalone prompt.
    pub description: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Repository-relative paths this task expects to touch.
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub role: TaskRole,
}

/// A validated decomposition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub tasks: Vec<TaskSpec>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PlanError {
    #[error("the plan has no tasks")]
    Empty,

    #[error("the plan has {count} tasks; the maximum is {max}")]
    TooLarge { count: usize, max: usize },

    #[error("duplicate task key {key:?}")]
    DuplicateKey { key: String },

    #[error("task key {key:?} is invalid: use lowercase letters, digits and dashes")]
    InvalidKey { key: String },

    #[error("task {key:?} has an empty title")]
    EmptyTitle { key: String },

    #[error("task {key:?} has an empty description")]
    EmptyDescription { key: String },

    #[error("task {key:?} depends on {dependency:?}, which does not exist")]
    UnknownDependency { key: String, dependency: String },

    #[error("dependency cycle: {chain}")]
    Cycle { chain: String },

    #[error("task {key:?} claims path {path:?}, which escapes the repository")]
    InvalidPath { key: String, path: String },

    #[error("task {key:?} claims an absolute path {path:?}")]
    AbsolutePath { key: String, path: String },

    #[error("task {key:?} claims a path with a wildcard segment {path:?}")]
    WildcardSegment { key: String, path: String },
}

impl Plan {
    /// Rejects everything the scheduler would have to guess about later.
    pub fn validate(&self) -> Result<(), PlanError> {
        if self.tasks.is_empty() {
            return Err(PlanError::Empty);
        }
        if self.tasks.len() > MAX_PLAN_TASKS {
            return Err(PlanError::TooLarge {
                count: self.tasks.len(),
                max: MAX_PLAN_TASKS,
            });
        }

        let mut keys = BTreeSet::new();
        for task in &self.tasks {
            if task.key.trim().is_empty()
                || !task
                    .key
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err(PlanError::InvalidKey {
                    key: task.key.clone(),
                });
            }
            if !keys.insert(task.key.clone()) {
                return Err(PlanError::DuplicateKey {
                    key: task.key.clone(),
                });
            }
            if task.title.trim().is_empty() {
                return Err(PlanError::EmptyTitle {
                    key: task.key.clone(),
                });
            }
            if task.description.trim().is_empty() {
                return Err(PlanError::EmptyDescription {
                    key: task.key.clone(),
                });
            }
            for path in &task.paths {
                validate_claim(&task.key, path)?;
            }
        }

        for task in &self.tasks {
            for dependency in &task.depends_on {
                if !keys.contains(dependency) {
                    return Err(PlanError::UnknownDependency {
                        key: task.key.clone(),
                        dependency: dependency.clone(),
                    });
                }
            }
        }

        detect_cycle(&self.tasks)?;
        Ok(())
    }

    /// Keys that no other task depends on. A plan with several leaves is a fan-out.
    pub fn leaves(&self) -> Vec<&str> {
        let mut depended: BTreeSet<&str> = BTreeSet::new();
        for task in &self.tasks {
            for dependency in &task.depends_on {
                depended.insert(dependency.as_str());
            }
        }
        self.tasks
            .iter()
            .map(|task| task.key.as_str())
            .filter(|key| !depended.contains(key))
            .collect()
    }

    /// Tasks whose dependency is `key`, for diagnostics.
    pub fn dependents(&self, key: &str) -> Vec<&str> {
        self.tasks
            .iter()
            .filter(|task| task.depends_on.iter().any(|dep| dep == key))
            .map(|task| task.key.as_str())
            .collect()
    }

    /// Topological order, used for display and for deterministic creation.
    pub fn topological(&self) -> Vec<&TaskSpec> {
        let by_key: BTreeMap<&str, &TaskSpec> = self
            .tasks
            .iter()
            .map(|task| (task.key.as_str(), task))
            .collect();
        let mut visited: BTreeSet<&str> = BTreeSet::new();
        let mut ordered: Vec<&TaskSpec> = Vec::new();

        fn visit<'a>(
            key: &'a str,
            by_key: &BTreeMap<&'a str, &'a TaskSpec>,
            visited: &mut BTreeSet<&'a str>,
            ordered: &mut Vec<&'a TaskSpec>,
        ) {
            if !visited.insert(key) {
                return;
            }
            if let Some(task) = by_key.get(key) {
                for dependency in &task.depends_on {
                    visit(dependency, by_key, visited, ordered);
                }
                ordered.push(task);
            }
        }

        for task in &self.tasks {
            visit(&task.key, &by_key, &mut visited, &mut ordered);
        }
        ordered
    }
}

fn validate_claim(key: &str, path: &str) -> Result<(), PlanError> {
    if path.trim().is_empty() {
        return Err(PlanError::InvalidPath {
            key: key.to_string(),
            path: path.to_string(),
        });
    }
    if path.starts_with('/') || path.starts_with('~') {
        return Err(PlanError::AbsolutePath {
            key: key.to_string(),
            path: path.to_string(),
        });
    }
    if path
        .split('/')
        .any(|segment| segment == ".." || segment.is_empty())
    {
        return Err(PlanError::InvalidPath {
            key: key.to_string(),
            path: path.to_string(),
        });
    }
    // Claims are prefixes/globs, not arbitrary shell patterns. A `**` inside a segment
    // (e.g. `src/**/x`) is allowed; a bare `**` segment is not, because it would claim
    // the whole repository and defeat the purpose of the claim.
    if path.split('/').any(|segment| segment == "**") && !path.starts_with("**") {
        return Err(PlanError::WildcardSegment {
            key: key.to_string(),
            path: path.to_string(),
        });
    }
    if path == "**" || path == "*" {
        return Err(PlanError::WildcardSegment {
            key: key.to_string(),
            path: path.to_string(),
        });
    }
    Ok(())
}

fn detect_cycle(tasks: &[TaskSpec]) -> Result<(), PlanError> {
    let by_key: BTreeMap<&str, &TaskSpec> =
        tasks.iter().map(|task| (task.key.as_str(), task)).collect();

    // 0 = unvisited, 1 = on the stack, 2 = done
    let mut state: BTreeMap<&str, u8> = BTreeMap::new();
    let mut stack: Vec<&str> = Vec::new();

    fn visit<'a>(
        key: &'a str,
        by_key: &BTreeMap<&'a str, &'a TaskSpec>,
        state: &mut BTreeMap<&'a str, u8>,
        stack: &mut Vec<&'a str>,
    ) -> Result<(), PlanError> {
        match state.get(key).copied().unwrap_or(0) {
            2 => return Ok(()),
            1 => {
                let position = stack.iter().position(|entry| *entry == key).unwrap_or(0);
                let mut chain: Vec<String> = stack[position..]
                    .iter()
                    .map(|entry| entry.to_string())
                    .collect();
                chain.push(key.to_string());
                return Err(PlanError::Cycle {
                    chain: chain.join(" -> "),
                });
            }
            _ => {}
        }

        state.insert(key, 1);
        stack.push(key);
        if let Some(task) = by_key.get(key) {
            for dependency in &task.depends_on {
                visit(dependency, by_key, state, stack)?;
            }
        }
        stack.pop();
        state.insert(key, 2);
        Ok(())
    }

    for task in tasks {
        visit(&task.key, &by_key, &mut state, &mut stack)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(key: &str, deps: &[&str]) -> TaskSpec {
        TaskSpec {
            key: key.to_string(),
            title: format!("task {key}"),
            description: format!("do {key}"),
            depends_on: deps.iter().map(|dep| dep.to_string()).collect(),
            paths: vec![format!("src/{key}.rs")],
            role: TaskRole::Implement,
        }
    }

    fn plan(tasks: Vec<TaskSpec>) -> Plan {
        Plan { tasks }
    }

    #[test]
    fn a_valid_plan_passes() {
        let plan = plan(vec![task("a", &[]), task("b", &["a"]), task("c", &["a"])]);
        plan.validate().unwrap();
        assert_eq!(plan.leaves(), vec!["b", "c"]);
        assert_eq!(plan.dependents("a"), vec!["b", "c"]);
    }

    #[test]
    fn the_topological_order_puts_dependencies_first() {
        let plan = plan(vec![task("b", &["a"]), task("a", &[])]);
        let ordered: Vec<&str> = plan
            .topological()
            .iter()
            .map(|task| task.key.as_str())
            .collect();
        assert_eq!(ordered, vec!["a", "b"]);
    }

    #[test]
    fn empty_and_oversized_plans_are_rejected() {
        assert_eq!(plan(vec![]).validate(), Err(PlanError::Empty));

        let tasks: Vec<TaskSpec> = (0..MAX_PLAN_TASKS + 1)
            .map(|index| task(&format!("t{index}"), &[]))
            .collect();
        assert!(matches!(
            plan(tasks).validate(),
            Err(PlanError::TooLarge { .. })
        ));
    }

    #[test]
    fn duplicate_and_invalid_keys_are_rejected() {
        assert!(matches!(
            plan(vec![task("a", &[]), task("a", &[])]).validate(),
            Err(PlanError::DuplicateKey { .. })
        ));
        assert!(matches!(
            plan(vec![task("Has Space", &[])]).validate(),
            Err(PlanError::InvalidKey { .. })
        ));
    }

    #[test]
    fn unknown_dependencies_are_rejected() {
        assert!(matches!(
            plan(vec![task("a", &["ghost"])]).validate(),
            Err(PlanError::UnknownDependency { .. })
        ));
    }

    #[test]
    fn cycles_are_rejected_with_the_chain() {
        let error = plan(vec![task("a", &["b"]), task("b", &["a"])])
            .validate()
            .unwrap_err();
        match error {
            PlanError::Cycle { chain } => {
                assert!(chain.contains("a"), "{chain}");
                assert!(chain.contains("b"), "{chain}");
            }
            other => panic!("expected a cycle, got {other:?}"),
        }
    }

    #[test]
    fn path_claims_cannot_escape_or_claim_the_world() {
        let mut escaping = task("a", &[]);
        escaping.paths = vec!["../etc/passwd".into()];
        assert!(matches!(
            plan(vec![escaping]).validate(),
            Err(PlanError::InvalidPath { .. })
        ));

        let mut absolute = task("a", &[]);
        absolute.paths = vec!["/etc/passwd".into()];
        assert!(matches!(
            plan(vec![absolute]).validate(),
            Err(PlanError::AbsolutePath { .. })
        ));

        let mut everything = task("a", &[]);
        everything.paths = vec!["**".into()];
        assert!(matches!(
            plan(vec![everything]).validate(),
            Err(PlanError::WildcardSegment { .. })
        ));
    }

    #[test]
    fn empty_titles_and_descriptions_are_rejected() {
        let mut empty_title = task("a", &[]);
        empty_title.title = "  ".into();
        assert!(matches!(
            plan(vec![empty_title]).validate(),
            Err(PlanError::EmptyTitle { .. })
        ));

        let mut empty_description = task("a", &[]);
        empty_description.description = "".into();
        assert!(matches!(
            plan(vec![empty_description]).validate(),
            Err(PlanError::EmptyDescription { .. })
        ));
    }

    #[test]
    fn plans_roundtrip_through_json() {
        let plan = plan(vec![task("a", &[]), task("b", &["a"])]);
        let json = serde_json::to_string(&plan).unwrap();
        let back: Plan = serde_json::from_str(&json).unwrap();
        assert_eq!(back, plan);
    }
}
