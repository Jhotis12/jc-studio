//! Canonical filesystem locations, shared by the daemon and the client.
//!
//! One source of truth: if the daemon and the CLI disagreed about the socket path,
//! the client would silently start a second daemon or fail with a confusing error.

use std::path::PathBuf;

use crate::SOCKET_NAME;

/// Directory holding the socket and other runtime-only files.
///
/// Prefers `$XDG_RUNTIME_DIR` (cleared on logout, which is exactly right for a socket).
/// Falls back to a private directory under the data dir when it is unset, e.g. on macOS.
pub fn runtime_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        let dir = PathBuf::from(dir);
        if !dir.as_os_str().is_empty() {
            return dir.join("jcs");
        }
    }
    data_dir().join("run")
}

/// Directory holding durable state (database, worktrees, logs).
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .map(|dir| dir.join("jcs"))
        .unwrap_or_else(|| PathBuf::from(".jcs"))
}

/// Socket the daemon listens on. Overridable with `JCS_SOCKET`.
pub fn socket_path() -> PathBuf {
    std::env::var_os("JCS_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| runtime_dir().join(SOCKET_NAME))
}

/// SQLite database file. Overridable with `JCS_DB`.
pub fn database_path() -> PathBuf {
    std::env::var_os("JCS_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir().join("jcs.db"))
}

/// Root of task worktrees.
pub fn worktrees_dir() -> PathBuf {
    data_dir().join("worktrees")
}

/// Directory for JSON logs.
pub fn log_dir() -> PathBuf {
    data_dir().join("logs")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn socket_lives_under_the_runtime_directory() {
        let socket = socket_path();
        assert!(socket.ends_with("jcsd.sock"), "{socket:?}");
    }

    #[test]
    fn database_lives_under_the_data_directory() {
        assert!(database_path().ends_with("jcs.db"));
    }

    #[test]
    fn worktrees_live_outside_the_repository_data_root() {
        assert!(worktrees_dir().ends_with("worktrees"));
    }
}
