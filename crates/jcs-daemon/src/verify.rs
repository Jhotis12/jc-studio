//! Verification: executable gates plus architectural fitness functions.
//!
//! Both are evaluated in the task worktree and both block success. Gates run commands;
//! boundaries check the dependency graph. A project can pass every command and still violate
//! its architecture, which is exactly the failure a linter cannot catch.

use std::path::Path;

use anyhow::{anyhow, Result};
use jcs_core::{GateStatus, Project};
use jcs_gates::{GateOutcome, GateSet};
use jcs_memory::BoundaryViolation;

use crate::state::AppState;

pub struct Evaluation {
    pub gates: GateSet,
    pub outcomes: Vec<GateOutcome>,
    pub boundary_rules: usize,
    pub boundary_violations: Vec<BoundaryViolation>,
}

/// Compiles the project policy, runs its gates and checks its boundaries in `worktree`.
pub async fn evaluate(state: &AppState, project: &Project, worktree: &Path) -> Result<Evaluation> {
    let compiled = jcs_policy::compile(
        &project.root_path,
        &state.packs,
        &state.config.default_packs,
    )
    .map_err(|error| anyhow!("cannot compile the policy of {}: {error}", project.name))?;

    let gates = compiled.composition.gate_set();
    let outcomes = if gates.is_empty() {
        Vec::new()
    } else {
        jcs_gates::run_all(&gates, worktree).await
    };

    let boundary_rules = compiled.policy.boundaries.len();
    let boundary_violations = if boundary_rules == 0 {
        Vec::new()
    } else {
        jcs_memory::check_boundaries(worktree, &compiled.policy.boundaries)
            .map_err(|error| anyhow!("cannot check architectural boundaries: {error}"))?
    };

    Ok(Evaluation {
        gates,
        outcomes,
        boundary_rules,
        boundary_violations,
    })
}

impl Evaluation {
    pub fn passed(&self) -> bool {
        let gates_pass =
            self.gates.is_empty() || GateSet::all_required_passed(&self.outcomes, &self.gates);
        gates_pass && self.boundary_violations.is_empty()
    }

    pub fn failed_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .outcomes
            .iter()
            .filter(|outcome| outcome.failed())
            .map(|outcome| outcome.name.clone())
            .collect();

        if !self.boundary_violations.is_empty() {
            names.push("architecture".to_string());
        }
        names
    }

    /// Bounded, per-gate output for a repair prompt or an event payload.
    pub fn excerpt(&self, max: usize) -> String {
        let mut parts: Vec<String> = self
            .outcomes
            .iter()
            .filter(|outcome| outcome.failed())
            .map(|outcome| format!("### {}\n{}", outcome.name, outcome.excerpt))
            .collect();

        if !self.boundary_violations.is_empty() {
            parts.push(format!("### architecture\n{}", self.render_violations()));
        }

        let joined = parts.join("\n\n");
        if joined.chars().count() <= max {
            joined
        } else {
            joined.chars().take(max).collect::<String>() + "\n… (truncated)"
        }
    }

    fn render_violations(&self) -> String {
        self.boundary_violations
            .iter()
            .map(|violation| {
                format!(
                    "- {}:{}: `{}` violates {}",
                    violation.path, violation.line, violation.import, violation.rule
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Persists command gates and the architectural verdict against a run.
    pub async fn record(&self, store: &jcs_store::Store, run_id: &jcs_core::RunId) -> Result<()> {
        for outcome in &self.outcomes {
            store
                .add_gate_result(
                    run_id,
                    &outcome.name,
                    outcome.status,
                    outcome.exit_code,
                    outcome.duration_ms,
                    Some(&outcome.excerpt),
                )
                .await?;
        }

        if self.boundary_rules > 0 {
            let (status, excerpt) = if self.boundary_violations.is_empty() {
                (GateStatus::Passed, None)
            } else {
                (GateStatus::Failed, Some(self.render_violations()))
            };

            store
                .add_gate_result(run_id, "architecture", status, None, 0, excerpt.as_deref())
                .await?;
        }

        Ok(())
    }
}
