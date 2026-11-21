//! Gate evaluation, shared by task execution and the merge queue.
//!
//! Both need the same answer — "does this worktree satisfy the project's policy?" — and
//! both must record the answer. Keeping it in one place is what stops the merge path from
//! drifting into a weaker check than the execution path.

use std::path::Path;

use anyhow::{anyhow, Result};
use jcs_core::Project;
use jcs_gates::{GateOutcome, GateSet};

use crate::state::AppState;

pub struct Evaluation {
    pub gates: GateSet,
    pub outcomes: Vec<GateOutcome>,
}

/// Compiles the project policy and runs its gates in `worktree`.
///
/// A project without gates passes: "no policy" must not mean "nothing to verify", but the
/// built-in commit gates still run afterwards.
pub async fn evaluate(state: &AppState, project: &Project, worktree: &Path) -> Result<Evaluation> {
    let compiled = jcs_policy::compile(
        &project.root_path,
        &state.packs,
        &state.config.default_packs,
    )
    .map_err(|error| anyhow!("cannot compile the policy of {}: {error}", project.name))?;

    let gates = compiled.composition.gate_set();
    if gates.is_empty() {
        return Ok(Evaluation {
            gates,
            outcomes: Vec::new(),
        });
    }

    let outcomes = jcs_gates::run_all(&gates, worktree).await;
    Ok(Evaluation { gates, outcomes })
}

impl Evaluation {
    pub fn passed(&self) -> bool {
        self.gates.is_empty() || GateSet::all_required_passed(&self.outcomes, &self.gates)
    }

    pub fn failed_names(&self) -> Vec<String> {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.failed())
            .map(|outcome| outcome.name.clone())
            .collect()
    }

    /// Bounded, per-gate output for a repair prompt or an event payload.
    pub fn excerpt(&self, max: usize) -> String {
        let joined = self
            .outcomes
            .iter()
            .filter(|outcome| outcome.failed())
            .map(|outcome| format!("### {}\n{}", outcome.name, outcome.excerpt))
            .collect::<Vec<_>>()
            .join("\n\n");

        if joined.chars().count() <= max {
            joined
        } else {
            joined.chars().take(max).collect::<String>() + "\n… (truncated)"
        }
    }

    /// Persists every gate result against a run.
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
        Ok(())
    }
}
