//! Cost backfill for runs created before the daemon stored per-run pricing.
//!
//! OpenCode reports tokens but may report cost 0. New runs store the catalog rate and are
//! priced from tokens; this one-shot pass repairs the historic rows.

use jcs_core::RunId;

use crate::state::AppState;

pub fn spawn_backfill(state: AppState) {
    tokio::spawn(async move {
        let agent = match state.agent.get().await {
            Ok(agent) => agent,
            Err(_) => return,
        };
        let models = match agent.model_catalog().await {
            Ok(models) => models,
            Err(error) => {
                tracing::debug!(error = %error, "cost backfill skipped: no model catalog");
                return;
            }
        };
        let rows = match state.store.runs_without_cost().await {
            Ok(rows) => rows,
            Err(error) => {
                tracing::warn!(error = %error, "cannot list runs without cost");
                return;
            }
        };

        let mut updated = 0usize;
        for (run_id, model, tokens_in, tokens_out) in rows {
            let Some(info) = models.iter().find(|info| info.reference == model) else {
                continue;
            };
            let cost = (tokens_in.max(0) as f64 * info.cost_input
                + tokens_out.max(0) as f64 * info.cost_output)
                / 1_000_000.0;
            if cost <= 0.0 {
                continue;
            }
            let Ok(run_id) = RunId::parse(run_id) else {
                continue;
            };
            if state.store.set_run_cost(&run_id, cost).await.is_ok() {
                updated += 1;
            }
        }

        if updated > 0 {
            tracing::info!(
                runs = updated,
                "backfilled run costs from the model catalog"
            );
        }
    });
}
