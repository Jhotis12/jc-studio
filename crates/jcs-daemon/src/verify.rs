//! Verification: executable gates, architectural fitness functions and secret scanning.
//!
//! All three block success and all three run in the task worktree:
//!   * gates run commands the project chose;
//!   * boundaries check the dependency graph, which no linter sees;
//!   * secrets catch credentials before they become a commit.

use std::path::Path;

use anyhow::{anyhow, Result};
use jcs_core::{GateStatus, Project};
use jcs_gates::{GateOutcome, GateSet, ScanOptions, ScanTarget, SecretFinding};
use jcs_memory::BoundaryViolation;

use crate::state::AppState;

pub struct Evaluation {
    pub gates: GateSet,
    pub outcomes: Vec<GateOutcome>,
    pub boundary_rules: usize,
    pub boundary_violations: Vec<BoundaryViolation>,
    pub secrets_enabled: bool,
    pub secret_findings: Vec<SecretFinding>,
}

/// Compiles the project policy and runs every check in `worktree`.
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

    let secrets_enabled = compiled.policy.secrets.enabled;
    let secret_findings = if secrets_enabled {
        // Only what this task wrote can leak a credential. Scanning the whole tree
        // flagged pre-existing lockfile hashes and model identifiers, and agents then
        // "fixed" unrelated files to get past the gate.
        let changed = jcs_git::Repo::new(worktree)
            .working_changed_lines()
            .await
            .map_err(|error| anyhow!("cannot list the task's changed lines: {error}"))?;
        let targets: Vec<ScanTarget> = changed
            .into_iter()
            .map(|change| ScanTarget {
                path: change.path,
                lines: change.lines,
            })
            .collect();
        jcs_gates::scan_targets(
            &targets,
            &ScanOptions {
                allow_prefixes: compiled.policy.secrets.allow.clone(),
            },
        )
    } else {
        Vec::new()
    };

    Ok(Evaluation {
        gates,
        outcomes,
        boundary_rules,
        boundary_violations,
        secrets_enabled,
        secret_findings,
    })
}

impl Evaluation {
    pub fn passed(&self) -> bool {
        let gates_pass =
            self.gates.is_empty() || GateSet::all_required_passed(&self.outcomes, &self.gates);
        gates_pass && self.boundary_violations.is_empty() && self.secret_findings.is_empty()
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
        if !self.secret_findings.is_empty() {
            names.push("secrets".to_string());
        }
        names
    }

    /// Bounded, per-check output for a repair prompt or an event payload.
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
        if !self.secret_findings.is_empty() {
            parts.push(format!("### secrets\n{}", self.render_secrets()));
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

    fn render_secrets(&self) -> String {
        self.secret_findings
            .iter()
            .map(|finding| {
                format!(
                    "- {}:{}: possible {} `{}` — remove it and rotate the credential",
                    finding.path, finding.line, finding.kind, finding.excerpt
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Persists command gates plus the built-in checks against a run.
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

        if self.secrets_enabled {
            let (status, excerpt) = if self.secret_findings.is_empty() {
                (GateStatus::Passed, None)
            } else {
                (GateStatus::Failed, Some(self.render_secrets()))
            };
            store
                .add_gate_result(run_id, "secrets", status, None, 0, excerpt.as_deref())
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
