//! Verification gates for JC Studio.
//!
//! A gate is the only part of the policy system that can say "no". Everything else is
//! advice to the model; a gate is a command whose exit status decides whether work is
//! accepted.

pub mod model;
pub mod parser;
pub mod runner;
pub mod secrets;

pub use model::{
    Finding, GateDef, GateError, GateOutcome, GateSet, OrderedFloat, ParserKind, Severity,
};
pub use runner::run;
pub use secrets::{scan as scan_secrets, ScanOptions, SecretFinding};

/// Runs a set of gates sequentially, in name order.
///
/// Sequential on purpose: gates share a working tree and often share a compiler cache, so
/// running them in parallel trades deterministic output for speed and makes failures harder
/// to attribute.
pub async fn run_all(gates: &GateSet, cwd: &std::path::Path) -> Vec<GateOutcome> {
    let mut outcomes = Vec::with_capacity(gates.gates.len());
    for def in gates.iter() {
        tracing::info!(gate = %def.name, "running gate");
        let outcome = run(def, cwd).await;
        tracing::info!(
            gate = %outcome.name,
            status = %outcome.status,
            duration_ms = outcome.duration_ms,
            "gate finished"
        );
        outcomes.push(outcome);
    }
    outcomes
}
