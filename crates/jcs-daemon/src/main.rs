//! `jcsd` — the JC Studio daemon.
//!
//! Owns durable state, task execution and the event log. Clients are thin; this process
//! is the single writer (ADR 0001).

mod config;
mod error;
mod executor;
mod http;
mod memory_service;
mod merge;
mod planner;
mod reviewer;
mod scheduler;
mod state;
mod uds;
mod verify;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use jcs_core::EventKind;
use jcs_store::Store;
use serde_json::json;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::state::AppState;

#[derive(Debug, Parser)]
#[command(
    name = "jcsd",
    version,
    about = "JC Studio daemon: durable state and multi-agent orchestration"
)]
struct Args {
    /// Unix socket to listen on (default: $XDG_RUNTIME_DIR/jcs/jcsd.sock).
    #[arg(long, env = "JCS_SOCKET")]
    socket: Option<PathBuf>,

    /// SQLite database file (default: $XDG_DATA_HOME/jcs/jcs.db).
    #[arg(long, env = "JCS_DB")]
    database: Option<PathBuf>,

    /// Maximum number of tasks executing at once.
    #[arg(long, env = "JCS_MAX_CONCURRENT")]
    max_concurrent: Option<usize>,

    /// Default model as provider/model, when a request does not name one.
    #[arg(long, env = "JCS_MODEL")]
    model: Option<String>,

    /// Default agent name, when a request does not name one.
    #[arg(long, env = "JCS_AGENT")]
    agent: Option<String>,

    /// Log filter, e.g. `info` or `jcs_daemon=debug`.
    #[arg(long, default_value = "info", env = "JCS_LOG_LEVEL")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing(&args.log_level);

    let mut config = Config::from_env();
    if let Some(socket) = args.socket {
        config.socket_path = socket;
    }
    if let Some(database) = args.database {
        config.db_path = database;
    }
    if let Some(max) = args.max_concurrent {
        config.max_concurrent = max.max(1);
    }
    if args.model.is_some() {
        config.default_model = args.model;
    }
    if args.agent.is_some() {
        config.default_agent = args.agent;
    }

    std::fs::create_dir_all(&config.worktrees_dir).with_context(|| {
        format!(
            "cannot create the worktrees directory {}",
            config.worktrees_dir.display()
        )
    })?;
    restrict_dir(&config.worktrees_dir)?;

    // The lock file makes a second daemon on the same database fail loudly.
    let store = Arc::new(
        Store::open(&config.db_path)
            .await
            .with_context(|| format!("cannot open {}", config.db_path.display()))?,
    );

    let packs = jcs_policy::loader::load_all(config.packs_dir.as_deref())
        .context("cannot load policy packs")?;

    let embeddings = jcs_memory::Embeddings::from_env().map(Arc::new);
    if embeddings.is_none() {
        tracing::info!(
            "semantic search disabled; set JCS_EMBEDDING_URL and JCS_EMBEDDING_MODEL to enable it"
        );
    }

    let state = AppState::new(store, Arc::new(config.clone()), Arc::new(packs), embeddings);

    // Crash recovery: a `started` run means the previous process died mid-flight.
    // Recovery must never prevent the daemon from booting: refusing to start would leave
    // the operator with no way to inspect the state that needs recovery.
    let interrupted = match state
        .store
        .interrupt_stale_runs("daemon restarted while the run was in flight")
        .await
    {
        Ok(count) => count,
        Err(error) => {
            tracing::error!(error = %error, "cannot mark stale runs as interrupted");
            0
        }
    };
    let parked = match state
        .store
        .block_orphaned_tasks("daemon restarted while the task was in flight")
        .await
    {
        Ok(count) => count,
        Err(error) => {
            tracing::error!(
                error = %error,
                "cannot park orphaned tasks; they will still show as in-flight"
            );
            0
        }
    };
    if interrupted > 0 || parked > 0 {
        tracing::warn!(
            interrupted_runs = interrupted,
            parked_tasks = parked,
            "recovered from an unclean shutdown; inspect with `jcs status`"
        );
    }

    if let Err(error) = state.agent.get().await {
        // Not fatal: the daemon must serve status and history even when the backend is
        // down. Requests that need an agent fail with an actionable message.
        tracing::warn!(reason = %error.message, "agent backend unavailable at startup");
    }

    http::record_lifecycle(
        &state,
        EventKind::DaemonStarted,
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "pid": std::process::id(),
            "socket": config.socket_path.to_string_lossy(),
            "recovered_runs": interrupted,
            "parked_tasks": parked,
        }),
    )
    .await;

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        database = %config.db_path.display(),
        socket = %config.socket_path.display(),
        max_concurrent = config.max_concurrent,
        allow_shell = config.allow_shell,
        default_packs = ?config.default_packs,
        repair_cycles = config.max_repair_cycles,
        "starting jcsd"
    );

    // The scheduler is the only component that dispatches work. It resumes whatever the
    // previous process left pending, using durable state only.
    let scheduler = scheduler::spawn(state.clone());
    let sweeper = memory_service::spawn_sweeper(state.clone());

    let result = uds::serve(state.clone(), config.socket_path.clone(), shutdown_signal()).await;

    scheduler.abort();
    if let Some(sweeper) = sweeper {
        sweeper.abort();
    }

    http::record_lifecycle(
        &state,
        EventKind::DaemonStopping,
        json!({ "reason": "shutdown signal" }),
    )
    .await;

    if let Err(error) = state.store.checkpoint().await {
        tracing::warn!(error = %error, "cannot checkpoint the database on shutdown");
    }

    tracing::info!("jcsd stopped");
    result
}

fn init_tracing(level: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_writer(std::io::stderr)
        .init();
}

fn restrict_dir(path: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o700);
        std::fs::set_permissions(path, permissions)
            .with_context(|| format!("cannot restrict permissions on {}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Resolves when the process is asked to stop.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
