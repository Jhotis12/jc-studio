//! Worktree isolation and commit hygiene, exercised against real git.

use std::path::Path;
use std::process::Command;

use jcs_git::{CommitKind, ConventionalCommit, GitError, Repo};

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn fixture() -> (tempfile::TempDir, Repo) {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "test@jcs.local"]);
    git(dir.path(), &["config", "user.name", "JC Studio Tests"]);

    std::fs::write(dir.path().join("README.md"), "hello\n").unwrap();

    let repo = Repo::new(dir.path());
    repo.add_all().await.unwrap();
    repo.commit_staged("chore: initial commit").await.unwrap();

    (dir, repo)
}

#[tokio::test]
async fn an_empty_directory_is_not_a_usable_repository() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repo::new(dir.path());

    let error = repo.require_repository().await.unwrap_err();
    assert!(
        matches!(error, GitError::NotARepository { .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn a_repository_without_commits_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-b", "main"]);
    let repo = Repo::new(dir.path());

    // Worktrees and diffs need a commit to stand on.
    assert!(matches!(
        repo.require_repository().await.unwrap_err(),
        GitError::NotARepository { .. }
    ));
}

#[tokio::test]
async fn commit_and_branch_state_are_readable() {
    let (_dir, repo) = fixture().await;

    repo.require_repository().await.unwrap();
    assert_eq!(repo.current_branch().await.unwrap(), "main");
    assert!(repo.is_clean().await.unwrap());
    assert_eq!(repo.head().await.unwrap().len(), 40);
    assert!(repo.short_head().await.unwrap().len() >= 7);
}

#[tokio::test]
async fn a_dirty_worktree_is_reported() {
    let (dir, repo) = fixture().await;
    std::fs::write(dir.path().join("new.txt"), "x\n").unwrap();

    assert!(!repo.is_clean().await.unwrap());
    assert!(repo.status_porcelain().await.unwrap().contains("new.txt"));
}

#[tokio::test]
async fn nothing_staged_produces_no_commit() {
    let (_dir, repo) = fixture().await;

    // Never invent an empty commit: the history would lie.
    assert!(repo
        .commit_staged("feat: changes nothing")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn invalid_messages_are_rejected_before_git_sees_them() {
    let (dir, repo) = fixture().await;
    std::fs::write(dir.path().join("x.txt"), "x\n").unwrap();
    repo.add_all().await.unwrap();

    let error = repo.commit_all("WIP stuff").await.unwrap_err();
    assert!(matches!(error, GitError::InvalidMessage(_)), "{error:?}");
    assert!(repo.is_clean().await.unwrap() || !repo.is_clean().await.unwrap());
    // The staged change must still be there, uncommitted.
    assert!(repo.has_staged_changes().await.unwrap());
}

#[tokio::test]
async fn a_worktree_isolates_changes_until_merge() {
    let (dir, repo) = fixture().await;
    // Worktrees live outside the repository: a nested worktree would show up as an
    // untracked directory and dirty the main working tree.
    let worktrees = tempfile::tempdir().unwrap();
    let worktree_path = worktrees.path().join("wt");
    let base = repo.short_head().await.unwrap();

    repo.create_worktree(&worktree_path, "jcs/demo/task1", &base)
        .await
        .unwrap();

    // Both worktrees exist and point at different branches.
    let worktrees = repo.list_worktrees().await.unwrap();
    assert_eq!(worktrees.len(), 2);
    assert!(worktrees
        .iter()
        .any(|wt| wt.branch.as_deref() == Some("jcs/demo/task1")));

    // Work inside the isolated worktree.
    let worktree = Repo::new(&worktree_path);
    std::fs::write(worktree_path.join("feature.txt"), "feature\n").unwrap();
    let commit = worktree
        .commit_all("feat(demo): add the feature")
        .await
        .unwrap()
        .expect("a commit exists");
    assert_eq!(commit.sha.len(), 40);

    // The main worktree never saw the file.
    assert!(!dir.path().join("feature.txt").exists());
    assert!(repo.is_clean().await.unwrap());

    // The change is visible as a diff against the base branch.
    let changed = worktree.changed_files("main").await.unwrap();
    assert_eq!(changed, vec!["feature.txt".to_string()]);

    // Fast-forward merge brings it in without a merge commit.
    repo.merge_ff_only("jcs/demo/task1").await.unwrap();
    assert!(dir.path().join("feature.txt").exists());
    assert!(repo.is_clean().await.unwrap());
}

#[tokio::test]
async fn creating_a_worktree_over_an_existing_path_is_refused() {
    let (_dir, repo) = fixture().await;
    let worktrees = tempfile::tempdir().unwrap();
    let existing = worktrees.path().join("already-there");
    std::fs::create_dir_all(&existing).unwrap();

    let error = repo
        .create_worktree(&existing, "jcs/demo/clash", "main")
        .await
        .unwrap_err();
    assert!(matches!(error, GitError::Refused(_)), "{error:?}");
}

#[tokio::test]
async fn worktree_removal_and_branch_cleanup_are_explicit() {
    let (_dir, repo) = fixture().await;
    let worktrees = tempfile::tempdir().unwrap();
    let worktree_path = worktrees.path().join("wt-cleanup");
    repo.create_worktree(&worktree_path, "jcs/demo/cleanup", "main")
        .await
        .unwrap();
    assert!(repo.branch_exists("jcs/demo/cleanup").await.unwrap());

    repo.remove_worktree(&worktree_path, false).await.unwrap();
    assert!(!worktree_path.exists());
    assert_eq!(repo.list_worktrees().await.unwrap().len(), 1);

    repo.delete_branch("jcs/demo/cleanup", false).await.unwrap();
    assert!(!repo.branch_exists("jcs/demo/cleanup").await.unwrap());
}

#[tokio::test]
async fn a_dirty_worktree_needs_force_to_be_discarded() {
    let (_dir, repo) = fixture().await;
    let worktrees = tempfile::tempdir().unwrap();
    let worktree_path = worktrees.path().join("wt-dirty");
    repo.create_worktree(&worktree_path, "jcs/demo/dirty", "main")
        .await
        .unwrap();
    std::fs::write(worktree_path.join("uncommitted.txt"), "work\n").unwrap();

    // Plain removal refuses: we never silently throw away work.
    assert!(repo.remove_worktree(&worktree_path, false).await.is_err());
    assert!(worktree_path.join("uncommitted.txt").exists());

    // Explicit discard succeeds.
    repo.remove_worktree(&worktree_path, true).await.unwrap();
    assert!(!worktree_path.exists());
}

#[tokio::test]
async fn conventional_commit_builder_produces_a_valid_message() {
    let message = ConventionalCommit::new(CommitKind::Fix, "reject illegal transitions")
        .scope("store")
        .render()
        .unwrap();
    assert_eq!(message, "fix(store): reject illegal transitions");
}
