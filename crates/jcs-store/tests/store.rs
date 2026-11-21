//! Persistence guarantees: exclusivity, transactional events, legal transitions, recovery.

use std::path::PathBuf;

use jcs_core::{EventKind, GoalStatus, Project, ProjectId, RunStatus, TaskStatus};
use jcs_store::{store::Store, StoreError};
use jiff::Timestamp;

fn project_at(root: PathBuf) -> Project {
    Project {
        id: ProjectId::new(),
        name: "demo".to_string(),
        root_path: root,
        base_branch: "main".to_string(),
        created_at: Timestamp::now(),
    }
}

async fn fixture() -> (tempfile::TempDir, Store, Project) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("jcs.db")).await.unwrap();
    let project = project_at(dir.path().to_path_buf());
    store.insert_project(&project).await.unwrap();
    (dir, store, project)
}

#[tokio::test]
async fn a_second_store_cannot_open_the_same_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jcs.db");

    let first = Store::open(&path).await.unwrap();
    let error = Store::open(&path).await.unwrap_err();

    assert!(
        matches!(error, StoreError::Locked { .. }),
        "expected Locked, got {error:?}"
    );

    drop(first);
    // Releasing the handle must free the lock for the next owner.
    Store::open(&path).await.unwrap();
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("jcs.db")).await.unwrap();
    store.migrate().await.unwrap();
    store.migrate().await.unwrap();
    assert_eq!(store.integrity_check().await.unwrap(), "ok");
}

#[tokio::test]
async fn projects_roundtrip_and_reject_duplicates() {
    let (_dir, store, project) = fixture().await;

    let listed = store.list_projects().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, project.id);

    let duplicate_name = Project {
        id: ProjectId::new(),
        name: project.name.clone(),
        root_path: PathBuf::from("/somewhere/else"),
        base_branch: "main".into(),
        created_at: Timestamp::now(),
    };
    let error = store.insert_project(&duplicate_name).await.unwrap_err();
    assert!(matches!(error, StoreError::Conflict(_)), "{error:?}");
}

#[tokio::test]
async fn project_resolution_accepts_name_or_path() {
    let (_dir, store, project) = fixture().await;

    let by_name = store.resolve_project("demo").await.unwrap().unwrap();
    assert_eq!(by_name.id, project.id);

    let by_path = store
        .resolve_project(&project.root_path.to_string_lossy())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_path.id, project.id);

    assert!(store.resolve_project("nope").await.unwrap().is_none());
}

#[tokio::test]
async fn goal_and_task_transitions_are_enforced_by_the_store() {
    let (_dir, store, project) = fixture().await;
    let goal = store
        .create_goal(&project.id, "ship the thing")
        .await
        .unwrap();

    // A goal cannot jump straight to success.
    let illegal = store.set_goal_status(&goal.id, GoalStatus::Succeeded).await;
    assert!(illegal.is_err(), "pending -> succeeded must be rejected");

    store
        .set_goal_status(&goal.id, GoalStatus::Running)
        .await
        .unwrap();
    let succeeded = store
        .set_goal_status(&goal.id, GoalStatus::Succeeded)
        .await
        .unwrap();
    assert_eq!(succeeded.status, GoalStatus::Succeeded);

    // Terminal states are absorbing even at the storage layer.
    assert!(store
        .set_goal_status(&goal.id, GoalStatus::Running)
        .await
        .is_err());

    let task = store.create_task(&goal.id, "add validation").await.unwrap();
    assert!(store
        .set_task_status(&task.id, TaskStatus::Succeeded)
        .await
        .is_err());
    store
        .set_task_status(&task.id, TaskStatus::Running)
        .await
        .unwrap();
    store
        .set_task_status(&task.id, TaskStatus::Verifying)
        .await
        .unwrap();
    assert_eq!(
        store
            .set_task_status(&task.id, TaskStatus::Succeeded)
            .await
            .unwrap()
            .status,
        TaskStatus::Succeeded
    );

    // Retry from failed is explicit and counted.
    let retryable = store.create_task(&goal.id, "flaky").await.unwrap();
    store
        .set_task_status(&retryable.id, TaskStatus::Running)
        .await
        .unwrap();
    store
        .set_task_status(&retryable.id, TaskStatus::Failed)
        .await
        .unwrap();
    store
        .set_task_status(&retryable.id, TaskStatus::Pending)
        .await
        .unwrap();
}

#[tokio::test]
async fn run_attempts_are_dense_and_finish_exactly_once() {
    let (_dir, store, project) = fixture().await;
    let goal = store.create_goal(&project.id, "goal").await.unwrap();
    let task = store.create_task(&goal.id, "task").await.unwrap();

    let first = store.start_run(&task.id).await.unwrap();
    assert_eq!(first.attempt, 1);
    assert_eq!(first.status, RunStatus::Started);

    store
        .finish_run(&first.id, RunStatus::Succeeded, Some(0), None)
        .await
        .unwrap();

    // Finishing twice is a programming error, not a silent no-op.
    assert!(store
        .finish_run(&first.id, RunStatus::Failed, None, None)
        .await
        .is_err());

    let second = store.start_run(&task.id).await.unwrap();
    assert_eq!(second.attempt, 2);

    let runs = store.list_runs_by_task(&task.id).await.unwrap();
    assert_eq!(runs.len(), 2);
}

#[tokio::test]
async fn usage_updates_are_absolute_not_accumulated() {
    let (_dir, store, project) = fixture().await;
    let goal = store.create_goal(&project.id, "goal").await.unwrap();
    let task = store.create_task(&goal.id, "task").await.unwrap();
    let run = store.start_run(&task.id).await.unwrap();

    store.set_run_usage(&run.id, 0.10, 100, 50).await.unwrap();
    store.set_run_usage(&run.id, 0.25, 400, 120).await.unwrap();

    let reloaded = store.get_run(&run.id).await.unwrap().unwrap();
    assert!((reloaded.cost_usd - 0.25).abs() < f64::EPSILON);
    assert_eq!(reloaded.tokens_in, 400);
    assert_eq!(reloaded.tokens_out, 120);

    // Usage is frozen once the run is over.
    store
        .finish_run(&run.id, RunStatus::Succeeded, Some(0), None)
        .await
        .unwrap();
    store.set_run_usage(&run.id, 99.0, 1, 1).await.unwrap();
    let frozen = store.get_run(&run.id).await.unwrap().unwrap();
    assert!(frozen.cost_usd < 1.0);
}

#[tokio::test]
async fn crashed_runs_become_interrupted_and_retryable() {
    let (_dir, store, project) = fixture().await;
    let goal = store.create_goal(&project.id, "goal").await.unwrap();
    let task = store.create_task(&goal.id, "task").await.unwrap();
    let run = store.start_run(&task.id).await.unwrap();

    let recovered = store
        .interrupt_stale_runs("daemon restarted")
        .await
        .unwrap();
    assert_eq!(recovered, 1);

    let reloaded = store.get_run(&run.id).await.unwrap().unwrap();
    assert_eq!(reloaded.status, RunStatus::Interrupted);
    assert!(reloaded.is_retryable());
    assert!(reloaded.ended_at.is_some());

    // Nothing left to recover the second time.
    assert_eq!(store.interrupt_stale_runs("again").await.unwrap(), 0);
}

#[tokio::test]
async fn orphaned_tasks_are_parked_as_blocked_not_failed() {
    let (_dir, store, project) = fixture().await;
    let goal = store.create_goal(&project.id, "goal").await.unwrap();
    let running = store.create_task(&goal.id, "was running").await.unwrap();
    let verifying = store.create_task(&goal.id, "was verifying").await.unwrap();

    store
        .set_task_status(&running.id, TaskStatus::Running)
        .await
        .unwrap();
    store
        .set_task_status(&verifying.id, TaskStatus::Running)
        .await
        .unwrap();
    store
        .set_task_status(&verifying.id, TaskStatus::Verifying)
        .await
        .unwrap();

    let parked = store.block_orphaned_tasks("daemon died").await.unwrap();
    assert_eq!(parked, 2);

    // `Blocked`, not `Failed`: a killed process proves nothing about the work.
    assert_eq!(
        store.get_task(&running.id).await.unwrap().unwrap().status,
        TaskStatus::Blocked
    );
    assert_eq!(
        store.get_task(&verifying.id).await.unwrap().unwrap().status,
        TaskStatus::Blocked
    );

    // Idempotent: nothing left in flight.
    assert_eq!(store.block_orphaned_tasks("again").await.unwrap(), 0);
}

#[tokio::test]
async fn event_log_is_monotonic_and_resumable() {
    let (_dir, store, project) = fixture().await;
    let goal = store.create_goal(&project.id, "goal").await.unwrap();
    let task = store.create_task(&goal.id, "task").await.unwrap();

    let all = jcs_store::events_since(&store, 0, 100).await.unwrap();
    let seqs: Vec<i64> = all.iter().map(|event| event.seq).collect();
    assert!(seqs.windows(2).all(|pair| pair[0] < pair[1]), "{seqs:?}");

    // Resume strictly after a cursor: no duplicates, no omissions.
    let cursor = seqs[1];
    let resumed = jcs_store::events_since(&store, cursor, 100).await.unwrap();
    assert!(resumed.iter().all(|event| event.seq > cursor));
    assert_eq!(resumed.len(), all.len() - 2);

    // The log names the task it belongs to.
    let timeline = jcs_store::events_for_task(&store, &task.id, 100)
        .await
        .unwrap();
    assert!(timeline
        .iter()
        .any(|event| event.kind == EventKind::TaskCreated));
}

#[tokio::test]
async fn an_empty_gate_set_is_not_a_pass() {
    let (_dir, store, project) = fixture().await;
    let goal = store.create_goal(&project.id, "goal").await.unwrap();
    let task = store.create_task(&goal.id, "task").await.unwrap();
    let run = store.start_run(&task.id).await.unwrap();

    // "No gates ran" must never look green.
    assert!(!store.all_gates_passed(&run.id).await.unwrap());

    store
        .add_gate_result(
            &run.id,
            "build",
            jcs_core::GateStatus::Passed,
            Some(0),
            12,
            None,
        )
        .await
        .unwrap();
    assert!(store.all_gates_passed(&run.id).await.unwrap());

    store
        .add_gate_result(
            &run.id,
            "tests",
            jcs_core::GateStatus::Failed,
            Some(1),
            40,
            Some("1 failing test"),
        )
        .await
        .unwrap();
    assert!(!store.all_gates_passed(&run.id).await.unwrap());
}

#[tokio::test]
async fn deleting_a_project_cascades_state_but_preserves_the_audit_trail() {
    let (_dir, store, project) = fixture().await;
    let goal = store.create_goal(&project.id, "goal").await.unwrap();
    let task = store.create_task(&goal.id, "task").await.unwrap();
    let before = jcs_store::count_events(&store).await.unwrap();

    store.delete_project(&project.id).await.unwrap();

    assert!(store.get_project(&project.id).await.unwrap().is_none());
    assert!(store.get_task(&task.id).await.unwrap().is_none());
    // Deletion is itself audited, and the history survives it.
    assert!(jcs_store::count_events(&store).await.unwrap() > before);
}

#[tokio::test]
async fn backup_produces_a_readable_snapshot() {
    let (dir, store, _project) = fixture().await;
    let target = dir.path().join("backup.db");

    store.backup_to(&target).await.unwrap();

    // The snapshot is a real database with the same content.
    let snapshot = Store::open(&target).await.unwrap();
    assert_eq!(snapshot.project_count().await.unwrap(), 1);
    assert_eq!(snapshot.integrity_check().await.unwrap(), "ok");
}
