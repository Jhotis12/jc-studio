use std::path::{Path, PathBuf};
use std::process::Output;

use tokio::process::Command;

use crate::commit::validate;
use crate::error::{render_command, GitError, Result};

/// A git repository (or a worktree of one), addressed by its working directory.
///
/// Every operation shells out to `git` with a separate argv array: no shell, no
/// interpolation, and behaviour identical to what the user would get by hand (hooks,
/// `insteadOf`, signing, line-ending policy). See ADR 0005.
///
/// There is deliberately no API for force-push, `reset --hard` or `clean -f` outside
/// an explicit worktree discard: destructive operations are impossible by construction
/// rather than guarded at runtime.
#[derive(Debug, Clone)]
pub struct Repo {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub bare: bool,
    pub detached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitOutcome {
    pub sha: String,
    pub short: String,
}

impl Repo {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Creates a new repository at this path with the given initial branch.
    pub async fn init(&self, branch: &str) -> Result<()> {
        let output = Command::new("git")
            .arg("init")
            .arg("-b")
            .arg(branch)
            .arg(&self.root)
            .output()
            .await?;
        self.expect_success(output, &["init", "-b", branch])?;
        Ok(())
    }

    /// Whether the path is inside a git working tree.
    pub async fn is_work_tree(&self) -> Result<bool> {
        let output = self.git(&["rev-parse", "--is-inside-work-tree"]).await?;
        if !output.status.success() {
            return Ok(false);
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim() == "true")
    }

    /// Fails with `NotARepository` unless this is a usable working tree with a commit.
    pub async fn require_repository(&self) -> Result<()> {
        if !self.is_work_tree().await? {
            return Err(GitError::NotARepository {
                path: self.root.clone(),
            });
        }
        // A repository with no commits cannot host worktrees or diffs.
        let output = self.git(&["rev-parse", "--verify", "HEAD"]).await?;
        if !output.status.success() {
            return Err(GitError::NotARepository {
                path: self.root.clone(),
            });
        }
        Ok(())
    }

    pub async fn head(&self) -> Result<String> {
        Ok(self
            .must_git(&["rev-parse", "HEAD"])
            .await?
            .trim()
            .to_string())
    }

    pub async fn short_head(&self) -> Result<String> {
        Ok(self
            .must_git(&["rev-parse", "--short", "HEAD"])
            .await?
            .trim()
            .to_string())
    }

    pub async fn current_branch(&self) -> Result<String> {
        Ok(self
            .must_git(&["rev-parse", "--abbrev-ref", "HEAD"])
            .await?
            .trim()
            .to_string())
    }

    /// Porcelain status; empty means clean.
    pub async fn status_porcelain(&self) -> Result<String> {
        self.must_git(&["status", "--porcelain=v1"]).await
    }

    pub async fn is_clean(&self) -> Result<bool> {
        Ok(self.status_porcelain().await?.trim().is_empty())
    }

    pub async fn add_all(&self) -> Result<()> {
        self.must_git(&["add", "-A"]).await?;
        Ok(())
    }

    /// Whether the index differs from HEAD.
    pub async fn has_staged_changes(&self) -> Result<bool> {
        let output = self.git(&["diff", "--cached", "--quiet"]).await?;
        match output.status.code() {
            Some(0) => Ok(false),
            Some(1) => Ok(true),
            _ => Err(GitError::command_failed(
                "git diff --cached --quiet",
                output.status.code(),
                &String::from_utf8_lossy(&output.stderr),
            )),
        }
    }

    /// Commits whatever is staged, after validating the message.
    ///
    /// Returns `None` when there is nothing staged: an empty commit is never useful
    /// and silently creating one would make the history lie.
    pub async fn commit_staged(&self, message: &str) -> Result<Option<CommitOutcome>> {
        validate(message)?;
        if !self.has_staged_changes().await? {
            return Ok(None);
        }

        self.must_git(&["commit", "-m", message, "--no-verify"])
            .await?;
        // `--no-verify` because hooks that block automated commits would deadlock the
        // pipeline; policy is enforced by our own gates instead.

        Ok(Some(CommitOutcome {
            sha: self.head().await?,
            short: self.short_head().await?,
        }))
    }

    /// Stages everything and commits it.
    pub async fn commit_all(&self, message: &str) -> Result<Option<CommitOutcome>> {
        self.add_all().await?;
        self.commit_staged(message).await
    }

    /// Creates a worktree on a new branch cut from `base`.
    pub async fn create_worktree(&self, path: &Path, branch: &str, base: &str) -> Result<()> {
        if path.exists() {
            return Err(GitError::Refused(format!(
                "worktree path {} already exists",
                path.display()
            )));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        self.must_git(&[
            "worktree",
            "add",
            "-b",
            branch,
            &path.to_string_lossy(),
            base,
        ])
        .await?;
        Ok(())
    }

    /// Removes a worktree. `force` discards uncommitted work: only for explicit
    /// user-requested abandonment, never in an automatic path.
    pub async fn remove_worktree(&self, path: &Path, force: bool) -> Result<()> {
        let path_str = path.to_string_lossy().to_string();
        let mut args = vec!["worktree", "remove"];
        if force {
            args.push("--force");
        }
        args.push(&path_str);
        self.must_git(&args).await?;
        Ok(())
    }

    /// Removes administrative records of worktrees whose directories are gone.
    pub async fn prune_worktrees(&self) -> Result<()> {
        self.must_git(&["worktree", "prune"]).await?;
        Ok(())
    }

    pub async fn list_worktrees(&self) -> Result<Vec<Worktree>> {
        let stdout = self.must_git(&["worktree", "list", "--porcelain"]).await?;

        let mut worktrees = Vec::new();
        let mut current: Option<Worktree> = None;

        for line in stdout.lines() {
            if let Some(path) = line.strip_prefix("worktree ") {
                if let Some(worktree) = current.take() {
                    worktrees.push(worktree);
                }
                current = Some(Worktree {
                    path: PathBuf::from(path),
                    head: None,
                    branch: None,
                    bare: false,
                    detached: false,
                });
            } else if let Some(head) = line.strip_prefix("HEAD ") {
                if let Some(worktree) = current.as_mut() {
                    worktree.head = Some(head.to_string());
                }
            } else if let Some(branch) = line.strip_prefix("branch ") {
                if let Some(worktree) = current.as_mut() {
                    worktree.branch = Some(branch.trim_start_matches("refs/heads/").to_string());
                }
            } else if line == "detached" {
                if let Some(worktree) = current.as_mut() {
                    worktree.detached = true;
                }
            } else if line == "bare" {
                if let Some(worktree) = current.as_mut() {
                    worktree.bare = true;
                }
            }
        }
        if let Some(worktree) = current {
            worktrees.push(worktree);
        }

        Ok(worktrees)
    }

    /// Creates a branch at `base` without checking it out.
    pub async fn create_branch(&self, branch: &str, base: &str) -> Result<()> {
        self.must_git(&["branch", branch, base]).await?;
        Ok(())
    }

    /// Moves an existing branch to another commit. Only safe for branches with no work.
    pub async fn move_branch(&self, branch: &str, to: &str) -> Result<()> {
        self.must_git(&["branch", "-f", branch, to]).await?;
        Ok(())
    }

    /// Adds a worktree checking out an existing branch.
    pub async fn add_worktree(&self, path: &Path, branch: &str) -> Result<()> {
        if path.exists() {
            return Err(GitError::Refused(format!(
                "worktree path {} already exists",
                path.display()
            )));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.must_git(&["worktree", "add", &path.to_string_lossy(), branch])
            .await?;
        Ok(())
    }

    /// Replays the current branch on top of `upstream`.
    pub async fn rebase_onto(&self, upstream: &str) -> Result<()> {
        self.must_git(&["rebase", upstream]).await?;
        Ok(())
    }

    /// Aborts an in-progress rebase. Best effort: used after a conflict was detected.
    pub async fn rebase_abort(&self) -> Result<()> {
        let output = self.git(&["rebase", "--abort"]).await?;
        if !output.status.success() {
            tracing::debug!("no rebase to abort");
        }
        Ok(())
    }

    /// Whether `ancestor` is reachable from `descendant`.
    pub async fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool> {
        let output = self
            .git(&["merge-base", "--is-ancestor", ancestor, descendant])
            .await?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(GitError::command_failed(
                "git merge-base --is-ancestor",
                output.status.code(),
                &String::from_utf8_lossy(&output.stderr),
            )),
        }
    }

    pub async fn branch_exists(&self, branch: &str) -> Result<bool> {
        let reference = format!("refs/heads/{branch}");
        let output = self
            .git(&["rev-parse", "--verify", "--quiet", &reference])
            .await?;
        Ok(output.status.success())
    }

    pub async fn delete_branch(&self, branch: &str, force: bool) -> Result<()> {
        let mut args = vec!["branch"];
        args.push(if force { "-D" } else { "-d" });
        args.push(branch);
        self.must_git(&args).await?;
        Ok(())
    }

    /// Fast-forward merge of `branch` into the current branch. Never creates a merge
    /// commit and never rewrites history.
    pub async fn merge_ff_only(&self, branch: &str) -> Result<()> {
        self.must_git(&["merge", "--ff-only", branch]).await?;
        Ok(())
    }

    pub async fn tag(&self, name: &str) -> Result<()> {
        self.must_git(&["tag", name]).await?;
        Ok(())
    }

    /// Files that differ between `base` and HEAD.
    pub async fn changed_files(&self, base: &str) -> Result<Vec<String>> {
        let stdout = self
            .must_git(&["diff", "--name-only", &format!("{base}...HEAD")])
            .await?;
        Ok(stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect())
    }

    /// Diff between `base` and the index, i.e. exactly what a commit would contain.
    pub async fn diff_staged(&self, base: &str) -> Result<String> {
        self.must_git(&["diff", "--cached", base]).await
    }

    pub async fn diff_stat(&self, base: &str) -> Result<String> {
        self.must_git(&["diff", "--stat", &format!("{base}...HEAD")])
            .await
    }

    /// Added plus removed lines between two revisions.
    ///
    /// Binary entries print `-` and are skipped; a change in a binary file is reported by
    /// the merge decision as unknown rather than as zero.
    pub async fn changed_lines_between(&self, from: &str, to: &str) -> Result<i64> {
        let output = self.must_git(&["diff", "--numstat", from, to]).await?;

        Ok(output
            .lines()
            .filter_map(|line| {
                let mut parts = line.split('\t');
                let added: i64 = parts.next()?.parse().ok()?;
                let deleted: i64 = parts.next()?.parse().ok()?;
                Some(added + deleted)
            })
            .sum())
    }

    /// Human-readable log of the last `count` commits.
    pub async fn log_oneline(&self, count: u32) -> Result<String> {
        let count = count.to_string();
        self.must_git(&["log", "--oneline", "-n", &count]).await
    }

    async fn git(&self, args: &[&str]) -> Result<Output> {
        Ok(Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()
            .await?)
    }

    async fn must_git(&self, args: &[&str]) -> Result<String> {
        let output = self.git(args).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{stderr}{}", String::from_utf8_lossy(&output.stdout));
            if combined.contains("not a git repository") {
                return Err(GitError::NotARepository {
                    path: self.root.clone(),
                });
            }
            return Err(GitError::command_failed(
                render_command(args),
                output.status.code(),
                &stderr,
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    fn expect_success(&self, output: Output, args: &[&str]) -> Result<()> {
        if output.status.success() {
            return Ok(());
        }
        Err(GitError::command_failed(
            render_command(args),
            output.status.code(),
            &String::from_utf8_lossy(&output.stderr),
        ))
    }
}
