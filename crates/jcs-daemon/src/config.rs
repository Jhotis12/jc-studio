use std::path::PathBuf;

use jcs_proto::paths;

/// Daemon configuration, resolved once at startup from the environment.
#[derive(Debug, Clone)]
pub struct Config {
    pub socket_path: PathBuf,
    pub db_path: PathBuf,
    pub worktrees_dir: PathBuf,
    /// Upper bound on concurrently executing tasks.
    pub max_concurrent: usize,
    /// Model used when a request does not specify one, as `provider/model`.
    pub default_model: Option<String>,
    pub default_agent: Option<String>,
    /// Whether the agent may run shell commands. Off by default: verification belongs
    /// to the gate runner, not to the process being verified.
    pub allow_shell: bool,
    /// Hard ceiling for a single run. A session that never reports an outcome must be
    /// stopped, not waited on forever.
    pub run_timeout_secs: u64,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            socket_path: paths::socket_path(),
            db_path: paths::database_path(),
            worktrees_dir: paths::worktrees_dir(),
            max_concurrent: env_parse("JCS_MAX_CONCURRENT").unwrap_or(2).max(1),
            default_model: env_non_empty("JCS_MODEL"),
            default_agent: env_non_empty("JCS_AGENT"),
            allow_shell: matches!(
                env_non_empty("JCS_ALLOW_SHELL").as_deref(),
                Some("1") | Some("true") | Some("yes")
            ),
            run_timeout_secs: env_parse("JCS_RUN_TIMEOUT_SECS").unwrap_or(900).max(30),
        }
    }
}

fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_parse<T: std::str::FromStr>(key: &str) -> Option<T> {
    env_non_empty(key).and_then(|value| value.parse().ok())
}
