use std::sync::Arc;
use std::time::Instant;

use jcs_agent::OpenCodeClient;
use jcs_store::Store;
use tokio::sync::{OnceCell, Semaphore};

use crate::config::Config;
use crate::error::ApiError;

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
    pub started_at: Instant,
    pub started_at_ms: i64,
    /// Bounds how many tasks execute at once.
    pub permits: Arc<Semaphore>,
}

impl AppState {
    pub fn new(store: Arc<Store>, config: Arc<Config>) -> Self {
        let permits = Arc::new(Semaphore::new(config.max_concurrent));
        Self {
            store,
            config,
            agent: AgentHandle::default(),
            started_at: Instant::now(),
            started_at_ms: jiff::Timestamp::now().as_millisecond(),
            permits,
        }
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
