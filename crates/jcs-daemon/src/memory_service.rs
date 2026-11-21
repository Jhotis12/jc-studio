//! Memory indexing service.
//!
//! Indexing is triggered by three events: a project is added, a goal is promoted into the
//! base branch, and a periodic sweep. All three go through the same guard, so two sweeps
//! cannot index the same project at once and compete for the same rows.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use jcs_core::Project;
use jcs_memory::{Embeddings, IndexReport};
use tokio::sync::Mutex;

use crate::state::AppState;

/// Prevents concurrent indexing of the same project.
#[derive(Debug, Clone, Default)]
pub struct IndexGuard {
    in_flight: Arc<Mutex<HashSet<String>>>,
}

impl IndexGuard {
    async fn begin(&self, project_id: &str) -> bool {
        let mut in_flight = self.in_flight.lock().await;
        in_flight.insert(project_id.to_string())
    }

    async fn end(&self, project_id: &str) {
        let mut in_flight = self.in_flight.lock().await;
        in_flight.remove(project_id);
    }
}

/// Indexes one project unless it is already being indexed.
pub async fn index_project(
    state: &AppState,
    project: &Project,
    force: bool,
) -> Option<IndexReport> {
    if !state.index_guard.begin(project.id.as_str()).await {
        tracing::debug!(project = %project.name, "index already in flight; skipping");
        return None;
    }

    let embeddings: Option<&Embeddings> = state.embeddings.as_deref();
    let result = jcs_memory::index_project(
        &state.store,
        embeddings,
        &project.id,
        &project.root_path,
        force,
    )
    .await;

    state.index_guard.end(project.id.as_str()).await;

    match result {
        Ok(report) => Some(report),
        Err(error) => {
            tracing::warn!(
                project = %project.name,
                error = %format!("{error:#}"),
                "memory indexing failed"
            );
            None
        }
    }
}

/// Indexes in the background; used where the caller must not wait for a disk sweep.
pub fn spawn_index(state: AppState, project: Project) {
    tokio::spawn(async move {
        index_project(&state, &project, false).await;
    });
}

/// Periodic incremental sweep.
///
/// The indexer hashes content, so an unchanged project costs a directory walk and no
/// parsing. That is cheap enough to run every few minutes and makes a filesystem watcher an
/// optimisation rather than a requirement.
pub fn spawn_sweeper(state: AppState) -> Option<tokio::task::JoinHandle<()>> {
    if state.config.memory_sweep_secs == 0 {
        return None;
    }

    let every = Duration::from_secs(state.config.memory_sweep_secs.max(60));

    Some(tokio::spawn(async move {
        loop {
            tokio::time::sleep(every).await;

            let projects = match state.store.list_projects().await {
                Ok(projects) => projects,
                Err(error) => {
                    tracing::warn!(error = %error, "cannot list projects for the memory sweep");
                    continue;
                }
            };

            for project in projects {
                index_project(&state, &project, false).await;
            }
        }
    }))
}
