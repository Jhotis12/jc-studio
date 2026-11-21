use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("{path} is not a git repository (or has no commits yet)")]
    NotARepository { path: PathBuf },

    #[error("git command failed{exit}: {message}", exit = .code.map(|c| format!(" (exit {c})")).unwrap_or_default())]
    Command {
        /// Rendered command line, for debugging. Never contains credentials.
        command: String,
        code: Option<i32>,
        message: String,
    },

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// An operation this crate deliberately does not allow.
    #[error("refused: {0}")]
    Refused(String),

    #[error("invalid conventional commit message: {0}")]
    InvalidMessage(String),
}

impl GitError {
    pub(crate) fn command_failed(
        command: impl Into<String>,
        code: Option<i32>,
        stderr: &str,
    ) -> Self {
        Self::Command {
            command: command.into(),
            code,
            message: stderr.trim().to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, GitError>;

/// Renders a command for diagnostics, quoting arguments that are not obviously safe.
pub(crate) fn render_command(args: &[&str]) -> String {
    let mut out = String::from("git");
    for arg in args {
        out.push(' ');
        let simple = arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-".contains(c));
        if simple {
            out.push_str(arg);
        } else {
            out.push('"');
            out.push_str(&arg.replace('"', "\\\""));
            out.push('"');
        }
    }
    out
}
