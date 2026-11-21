use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use jcs_agent::OpenCodeClient;
use jcs_memory::Embeddings;
use jcs_policy::model::Pack;
use jcs_store::Store;
use tokio::sync::{OnceCell, Semaphore};

use crate::config::Config;
use crate::error::ApiError;
use crate::settings::RuntimeSettings;

/// Lazily discovered backend client.
///
/// The daemon must start even when OpenCode is down: refusing to boot because a
/// dependency is offline would make `jcs status` and the audit log unavailable exactly
/// when the operator needs them. Discovery is retried until it succeeds.
#[derive(Clone, Default)]
pub struct AgentHandle {
    client: Arc<OnceCell<Arc<OpenCodeClient>>>,
}

impl AgentHandle {
    pub async fn get(&self) -> Result<Arc<OpenCodeClient>, ApiError> {
        if let Some(client) = self.client.get() {
            return Ok(client.clone());
        }

        match OpenCodeClient::discover() {
            Ok(client) => {
                let client = Arc::new(client);
                // A concurrent caller may have won the race; keep whichever landed.
                let _ = self.client.set(client.clone());
                Ok(self.client.get().cloned().unwrap_or(client))
            }
            Err(error) => Err(ApiError::unavailable(
                "agent_unavailable",
                format!(
                    "cannot reach the agent backend: {error}. \
                     Start it with `opencode service start`, or set JCS_OPENCODE_URL."
                ),
            )),
        }
    }
}

/// Shared, cloneable daemon state.
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub config: Arc<Config>,
    pub agent: AgentHandle,
    /// Packs available in this daemon: built-ins plus `JCS_PACKS_DIR`.
    pub packs: Arc<BTreeMap<String, Pack>>,
    /// Semantic search provider, when one is configured.
    pub embeddings: Option<Arc<Embeddings>>,
    /// Prevents two indexing passes over the same project at once.
    pub index_guard: crate::memory_service::IndexGuard,
    pub started_at: Instant,
    pub started_at_ms: i64,
    /// Bounds how many tasks execute at once.
    pub permits: Arc<Semaphore>,
    /// Values the operator can change from the interface.
    pub runtime: Arc<RuntimeSettings>,
}

impl AppState {
    pub fn new(
        store: Arc<Store>,
        config: Arc<Config>,
        packs: Arc<BTreeMap<String, Pack>>,
        embeddings: Option<Arc<Embeddings>>,
        runtime: Arc<RuntimeSettings>,
    ) -> Self {
        let permits = Arc::new(Semaphore::new(
            runtime.max_concurrent().try_into().unwrap_or(1),
        ));
        Self {
            store,
            config,
            agent: AgentHandle::default(),
            packs,
            embeddings,
            index_guard: crate::memory_service::IndexGuard::default(),
            started_at: Instant::now(),
            started_at_ms: jiff::Timestamp::now().as_millisecond(),
            permits,
            runtime,
        }
    }

    // The scheduler and executor read these on every task, so a change in the interface
    // applies to the next dispatch without restarting the daemon.

    pub fn default_model(&self) -> Option<String> {
        self.runtime.default_model()
    }

    pub fn default_agent(&self) -> Option<String> {
        self.runtime.default_agent()
    }

    pub fn planning_model(&self) -> Option<String> {
        self.runtime.planning_model()
    }

    pub fn planning_agent(&self) -> Option<String> {
        self.runtime.planning_agent()
    }

    pub fn review_model(&self) -> Option<String> {
        self.runtime.review_model()
    }

    pub fn review(&self) -> bool {
        self.runtime.review()
    }

    pub fn risk_max_lines(&self) -> u64 {
        self.runtime.risk_max_lines()
    }

    pub fn run_timeout_secs(&self) -> u64 {
        self.runtime.run_timeout_secs()
    }

    pub fn context_tokens(&self) -> u64 {
        self.runtime.context_tokens()
    }

    pub fn max_repair_cycles(&self) -> u64 {
        self.runtime.max_repair_cycles()
    }

    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("config", &self.config)
            .field("uptime_secs", &self.uptime_secs())
            .finish_non_exhaustive()
    }
}
