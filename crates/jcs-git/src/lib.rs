//! Git primitives for JC Studio.
//!
//! Worktree-per-task isolation and conventional commits, implemented by invoking the
//! `git` binary with explicit argv arrays (ADR 0005). No shell, no interpolation, and
//! no API for destructive history operations.

pub mod commit;
pub mod error;
pub mod repo;

pub use commit::{branch_name, slugify, validate, CommitKind, ConventionalCommit};
pub use error::{GitError, Result};
pub use repo::{CommitOutcome, Repo, Worktree};
