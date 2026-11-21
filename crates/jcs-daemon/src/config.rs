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
    /// Extra packs directory, overriding built-ins by id (`JCS_PACKS_DIR`).
    pub packs_dir: Option<PathBuf>,
    /// Packs used when a project has no `.jcs/policy.toml`. Empty means "no policy":
    /// only the built-in commit gates run.
    pub default_packs: Vec<String>,
    /// How many times a failed verification is sent back to the agent before escalating.
    pub max_repair_cycles: usize,
    /// Model used for planning; defaults to `default_model`.
    pub planning_model: Option<String>,
    pub planning_agent: Option<String>,
    /// How long a task lease survives without a heartbeat.
    pub lease_ttl_secs: u64,
    /// How often a running task renews its lease.
    pub heartbeat_secs: u64,
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
            packs_dir: std::env::var_os("JCS_PACKS_DIR").map(PathBuf::from),
            default_packs: env_non_empty("JCS_DEFAULT_PACKS")
                .map(|value| {
                    value
                        .split(',')
                        .map(|part| part.trim().to_string())
                        .filter(|part| !part.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            max_repair_cycles: env_parse("JCS_MAX_REPAIR_CYCLES").unwrap_or(2).min(5),
            planning_model: env_non_empty("JCS_PLANNING_MODEL"),
            planning_agent: env_non_empty("JCS_PLANNING_AGENT"),
            lease_ttl_secs: env_parse("JCS_LEASE_TTL_SECS").unwrap_or(90).max(10),
            heartbeat_secs: env_parse("JCS_HEARTBEAT_SECS").unwrap_or(20).max(2),
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
