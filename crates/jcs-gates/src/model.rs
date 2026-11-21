//! Gate definitions and outcomes.
//!
//! A gate is a command with a verdict. It is the only part of the policy system that can
//! say "no": everything else is advice to the model.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use jcs_core::GateStatus;

/// How a gate's output should be interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParserKind {
    /// The exit code is the whole verdict.
    #[default]
    ExitCode,
    /// Extract error-looking lines and keep the tail of the output.
    Generic,
    /// Rust toolchain output: `error[E...]`, `error:`, failed tests.
    Cargo,
    /// Node toolchain output: `error TS####`, ESLint/Jest failures.
    Node,
    /// Extract a coverage percentage and compare it with `min_coverage`.
    Coverage,
}

/// A verification command.
///
/// `command` is an argv array, never a shell string: packs cannot smuggle in quoting or
/// shell expansion, and a pack that genuinely needs a pipeline has to spell out
/// `["sh", "-c", "..."]` where a reviewer can see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateDef {
    pub name: String,
    pub command: Vec<String>,
    pub timeout: Duration,
    pub required: bool,
    pub parser: ParserKind,
    /// Enforced when `parser = Coverage`.
    pub min_coverage: Option<OrderedFloat>,
}

/// A float with `Eq`, so `GateDef` can derive `Eq` and be compared in tests.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OrderedFloat(pub f64);

impl Eq for OrderedFloat {}

impl std::fmt::Display for OrderedFloat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl GateDef {
    /// Validates the parts that would otherwise fail confusingly at execution time.
    pub fn validate(&self) -> Result<(), GateError> {
        if self.name.trim().is_empty() {
            return Err(GateError::Invalid("gate name is empty".into()));
        }
        if self.command.is_empty() {
            return Err(GateError::Invalid(format!(
                "gate {:?} has an empty command",
                self.name
            )));
        }
        if self.command[0].trim().is_empty() {
            return Err(GateError::Invalid(format!(
                "gate {:?} has an empty program",
                self.name
            )));
        }
        if self.timeout.is_zero() {
            return Err(GateError::Invalid(format!(
                "gate {:?} has a zero timeout",
                self.name
            )));
        }
        if self.timeout > Duration::from_secs(3_600) {
            return Err(GateError::Invalid(format!(
                "gate {:?} has a timeout above one hour",
                self.name
            )));
        }
        if self.min_coverage.is_some() && self.parser != ParserKind::Coverage {
            return Err(GateError::Invalid(format!(
                "gate {:?} sets min_coverage with a non-coverage parser",
                self.name
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// One actionable problem extracted from a gate's output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub severity: Severity,
    pub message: String,
    /// Best-effort `file:line`, when the parser can identify one.
    pub location: Option<String>,
}

/// What happened when a gate ran.
#[derive(Debug, Clone, PartialEq)]
pub struct GateOutcome {
    pub name: String,
    pub status: GateStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: i64,
    /// Bounded, human-readable output for the repair prompt and the event log.
    pub excerpt: String,
    pub findings: Vec<Finding>,
    /// True when the gate was killed for exceeding its timeout.
    pub timed_out: bool,
}

impl GateOutcome {
    pub fn failed(&self) -> bool {
        self.status.is_blocking()
    }
}

/// All gates that should run for a project, in execution order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GateSet {
    pub gates: BTreeMap<String, GateDef>,
}

impl GateSet {
    pub fn is_empty(&self) -> bool {
        self.gates.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &GateDef> {
        self.gates.values()
    }

    /// Whether every required gate passed. An empty set is **not** a pass.
    pub fn all_required_passed(outcomes: &[GateOutcome], gates: &GateSet) -> bool {
        if outcomes.is_empty() {
            return false;
        }
        outcomes.iter().all(|outcome| {
            let required = gates
                .gates
                .get(&outcome.name)
                .map(|gate| gate.required)
                .unwrap_or(true);
            !required || outcome.status == GateStatus::Passed
        })
    }
}

/// Resolves the working directory for a gate, which is always the task worktree.
pub fn ensure_directory(directory: &Path) -> Result<(), GateError> {
    if !directory.is_dir() {
        return Err(GateError::Invalid(format!(
            "gate working directory {} does not exist or is not a directory",
            directory.display()
        )));
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum GateError {
    #[error("invalid gate: {0}")]
    Invalid(String),

    #[error("cannot start {program}: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate(name: &str, command: &[&str]) -> GateDef {
        GateDef {
            name: name.to_string(),
            command: command.iter().map(|part| part.to_string()).collect(),
            timeout: Duration::from_secs(10),
            required: true,
            parser: ParserKind::ExitCode,
            min_coverage: None,
        }
    }

    #[test]
    fn empty_commands_are_rejected() {
        let mut def = gate("build", &[]);
        assert!(def.validate().is_err());

        def.command = vec!["".to_string()];
        assert!(def.validate().is_err());
    }

    #[test]
    fn zero_and_huge_timeouts_are_rejected() {
        let mut def = gate("build", &["cargo", "build"]);
        def.timeout = Duration::ZERO;
        assert!(def.validate().is_err());

        def.timeout = Duration::from_secs(7_200);
        assert!(def.validate().is_err());
    }

    #[test]
    fn coverage_threshold_only_makes_sense_with_the_coverage_parser() {
        let mut def = gate("coverage", &["cargo", "llvm-cov"]);
        def.min_coverage = Some(OrderedFloat(80.0));
        assert!(def.validate().is_err());

        def.parser = ParserKind::Coverage;
        assert!(def.validate().is_ok());
    }

    #[test]
    fn an_empty_gate_set_never_counts_as_success() {
        assert!(!GateSet::all_required_passed(&[], &GateSet::default()));
    }

    #[test]
    fn optional_gates_do_not_block_the_verdict() {
        let required = gate("build", &["true"]);
        let mut optional = gate("audit", &["false"]);
        optional.required = false;

        let mut gates = GateSet::default();
        gates.gates.insert(required.name.clone(), required);
        gates.gates.insert(optional.name.clone(), optional);

        let outcomes = vec![
            GateOutcome {
                name: "build".into(),
                status: GateStatus::Passed,
                exit_code: Some(0),
                duration_ms: 1,
                excerpt: String::new(),
                findings: Vec::new(),
                timed_out: false,
            },
            GateOutcome {
                name: "audit".into(),
                status: GateStatus::Failed,
                exit_code: Some(1),
                duration_ms: 1,
                excerpt: String::new(),
                findings: Vec::new(),
                timed_out: false,
            },
        ];

        assert!(GateSet::all_required_passed(&outcomes, &gates));
    }
}
