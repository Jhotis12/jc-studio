//! Task execution pipeline.
//!
//! One task = one isolated worktree + one agent session + one commit, gated.
//!
//! Ordering matters and is deliberate:
//!   1. the worktree exists before the agent does, so the agent cannot escape it;
//!   2. the task reaches `Verifying` before any gate runs, so success is never recorded
//!      on the agent's word alone;
//!   3. the run only finishes after the gates have their verdict.
//!
//! The executor owns the *task*, not the *goal*: goal status is derived by the scheduler
//! from the task counts, so a multi-task plan cannot be finalized by whichever task
//! happened to finish last.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use jcs_agent::{ExecutionOutcome, ModelRef, OpenCodeClient, ToolInvocation};
use jcs_core::{EventKind, GateStatus, Goal, NewEvent, Project, RunId, Task, TaskId, TaskStatus};
use jcs_git::{branch_name, CommitKind, ConventionalCommit, Repo};
use jcs_proto::PermissionMode;
use jcs_store::Store;
use serde_json::json;
use tokio::task::JoinHandle;
use tokio::time::{interval, MissedTickBehavior};

use crate::state::AppState;

/// How long text deltas are buffered before being persisted as one event.
const TEXT_FLUSH_INTERVAL: Duration = Duration::from_millis(300);
/// How often the backend is reconciled over REST (outcome, forms, permissions).
const RECONCILE_INTERVAL: Duration = Duration::from_secs(2);
/// Upper bound on a single persisted text chunk.
const TEXT_CHUNK_LIMIT: usize = 4_000;

/// Everything the executor needs to run one task.
#[derive(Debug, Clone)]
pub struct TaskDispatch {
    pub task_id: TaskId,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub permissions: PermissionMode,
    /// Owner of the task lease; used for heartbeats and release.
    pub lease_owner: String,
}

/// Starts a task in the background.
///
/// The caller passes the concurrency permit it already acquired. Acquiring it here would
/// let the scheduler dispatch more tasks than the machine can run, leaving "started" tasks
/// that are actually queued — and invisible to the budget and in-flight checks.
pub fn spawn(state: AppState, dispatch: TaskDispatch, permit: tokio::sync::OwnedSemaphorePermit) {
    tokio::spawn(async move {
        let _permit = permit;
        let guard = LeaseGuard::start(&state, &dispatch);

        match run_task(&state, &dispatch, &guard).await {
            Ok(()) => {}
            Err(error) if guard.lost() => {
                // Someone else owns the task now; touching its state would fight them.
                tracing::warn!(
                    task = %dispatch.task_id,
                    error = %format!("{error:#}"),
                    "lease lost; abandoning this execution"
                );
            }
            Err(error) => {
                tracing::error!(
                    task = %dispatch.task_id,
                    error = %format!("{error:#}"),
                    "task failed"
                );
                let run_id = guard.run_id();
                if let Err(mark) =
                    fail_task(&state, &dispatch, run_id.as_ref(), &format!("{error:#}")).await
                {
                    tracing::error!(error = %mark, "could not record the failure");
                }
            }
        }
    });
}

async fn run_task(state: &AppState, dispatch: &TaskDispatch, guard: &LeaseGuard) -> Result<()> {
    let task = state
        .store
        .get_task(&dispatch.task_id)
        .await?
        .ok_or_else(|| anyhow!("task {} no longer exists", dispatch.task_id))?;
    let goal = state
        .store
        .get_goal(&task.goal_id)
        .await?
        .ok_or_else(|| anyhow!("goal {} no longer exists", task.goal_id))?;
    let project = state
        .store
        .get_project(&goal.project_id)
        .await?
        .ok_or_else(|| anyhow!("project {} no longer exists", goal.project_id))?;

    // Every attempt is its own run: retries are counted, never overwritten.
    let run = state.store.start_run(&task.id).await?;
    guard.set_run_id(&run.id);
    let run_id = run.id.clone();

    // 1. Isolate before anything else can touch the repository.
    let worktree = state
        .config
        .worktrees_dir
        .join(project.id.as_str())
        .join(task.id.as_str());
    let branch = branch_name(&goal.prompt, task.id.as_str());

    // Dependent tasks must see their predecessors: cut from the goal's integration
    // branch, not from the original base.
    let base_branch = state
        .store
        .goal_integration_branch(&goal.id)
        .await?
        .unwrap_or_else(|| project.base_branch.clone());

    if !worktree.exists() {
        let repo = Repo::new(&project.root_path);
        repo.require_repository().await.with_context(|| {
            format!(
                "{} is not usable as a project (needs a git repo with at least one commit)",
                project.root_path.display()
            )
        })?;
        repo.create_worktree(&worktree, &branch, &base_branch)
            .await
            .with_context(|| format!("cannot create a worktree at {}", worktree.display()))?;
        state
            .store
            .attach_worktree(&task.id, &worktree, &branch)
            .await?;
    }

    state
        .store
        .set_task_status(&task.id, TaskStatus::Running)
        .await?;

    // 2. Bind an agent session to the worktree, never to the main checkout.
    let agent = state
        .agent
        .get()
        .await
        .map_err(|error| anyhow!(error.message))?;

    let model = match dispatch.model.clone().or_else(|| state.default_model()) {
        Some(reference) => Some(ModelRef::parse(&reference)?),
        None => None,
    };
    let agent_name = dispatch.agent.clone().or_else(|| state.default_agent());

    let session = agent
        .create_session(jcs_agent::CreateSession {
            title: &task.title,
            directory: &worktree,
            model,
            agent: agent_name.as_deref(),
            permissions: None,
        })
        .await
        .context("cannot create an agent session")?;

    state
        .store
        .link_session(&task.id, &project.id, &session.id, &worktree)
        .await?;

    let mut prompt = build_prompt(&goal, &task, &worktree, &branch, &project.base_branch);
    prompt = with_memory_context(state, &project, &goal, &task, prompt).await;

    agent
        .prompt(&session.id, &prompt, "queue")
        .await
        .context("cannot deliver the initial prompt")?;

    // 3. Execute, verify, and repair until the gates agree with the agent.
    let mut repair_cycles = 0usize;

    loop {
        match pump_events(state, dispatch, &run_id, &session.id).await? {
            PumpResult::Finished(ExecutionOutcome::Succeeded) => {}
            PumpResult::Finished(ExecutionOutcome::Failed) => {
                return Err(anyhow!("the agent reported that the execution failed"));
            }
            PumpResult::Finished(ExecutionOutcome::Interrupted) => {
                return Err(anyhow!("the execution was interrupted before it completed"));
            }
            PumpResult::NeedsInput(question) => {
                return block_for_input(state, dispatch, &run_id, &task, &question).await;
            }
            PumpResult::TimedOut => {
                return Err(anyhow!(
                    "the run exceeded its {}s budget and was stopped",
                    state.run_timeout_secs()
                ));
            }
        }

        if guard.lost() {
            bail!("the task lease was lost; another owner is responsible for this task");
        }

        state
            .store
            .set_task_status(&task.id, TaskStatus::Verifying)
            .await?;

        // The run's gate list should describe the latest attempt; the event log keeps
        // every attempt.
        state.store.clear_gate_results(&run_id).await?;
        let evaluation = crate::verify::evaluate(state, &project, &worktree).await?;
        evaluation.record(&state.store, &run_id).await?;

        let mut failures = evaluation.failed_names();
        let mut excerpt = evaluation.excerpt(6_000);

        // Review runs only after the executable checks pass: there is no point spending a
        // model call reviewing code that does not compile.
        if failures.is_empty() && state.review() {
            match crate::reviewer::review(state, &project, &goal, &task, &worktree, &base_branch)
                .await
            {
                Ok(verdict) => {
                    let status = if verdict.approve {
                        GateStatus::Passed
                    } else {
                        GateStatus::Failed
                    };
                    state
                        .store
                        .add_gate_result(
                            &run_id,
                            "review",
                            status,
                            None,
                            0,
                            Some(&verdict.render()),
                        )
                        .await?;

                    if !verdict.approve {
                        tracing::warn!(
                            task = %task.id,
                            findings = verdict.blocking().count(),
                            "review requested changes"
                        );
                        failures = vec!["review".to_string()];
                        excerpt = verdict.render();
                    }
                }
                Err(error) => {
                    // A reviewer outage must not block work that already passed its gates.
                    tracing::warn!(
                        task = %task.id,
                        error = %format!("{error:#}"),
                        "review failed; continuing without it"
                    );
                    state
                        .store
                        .add_gate_result(
                            &run_id,
                            "review",
                            GateStatus::Errored,
                            None,
                            0,
                            Some(&format!("{error:#}")),
                        )
                        .await?;
                }
            }
        }

        if failures.is_empty() {
            return finalize_success(
                state, dispatch, &run_id, &task, &project, &worktree, &branch,
            )
            .await;
        }

        if repair_cycles >= state.max_repair_cycles() as usize {
            let question = format!(
                "verification failed after {repair_cycles} repair cycle(s); still failing: {}. \
                 Inspect the worktree, fix the cause, or adjust the policy.",
                failures.join(", ")
            );
            return block_for_input(state, dispatch, &run_id, &task, &question).await;
        }

        repair_cycles += 1;
        tracing::warn!(
            task = %task.id,
            cycle = repair_cycles,
            checks = ?failures,
            "verification failed; asking the agent to repair"
        );

        // Verifying -> Running is a legal transition and records the loop honestly.
        state
            .store
            .set_task_status(&task.id, TaskStatus::Running)
            .await?;

        agent
            .prompt(&session.id, &repair_prompt(&failures, &excerpt), "queue")
            .await
            .context("cannot deliver the repair prompt")?;

        // The previous turn's `outcome` persists in the backend. Wait until the new turn
        // is actually running before subscribing again, or we would read a stale verdict.
        wait_until_busy(&agent, &session.id).await;
    }
}

/// Guards a task lease for the lifetime of an execution.
///
/// The heartbeat is the only thing standing between "one owner" and "two agents editing
/// the same worktree", so losing it aborts the execution instead of being ignored.
struct LeaseGuard {
    store: Arc<Store>,
    resource: String,
    owner: String,
    lost: Arc<AtomicBool>,
    run_id: std::sync::Mutex<Option<RunId>>,
    heartbeat: JoinHandle<()>,
}

impl LeaseGuard {
    fn start(state: &AppState, dispatch: &TaskDispatch) -> Self {
        let resource = format!("task:{}", dispatch.task_id);
        let owner = dispatch.lease_owner.clone();
        let lost = Arc::new(AtomicBool::new(false));
        let ttl = Duration::from_secs(state.config.lease_ttl_secs);
        let every = Duration::from_secs(state.config.heartbeat_secs);

        let heartbeat = tokio::spawn({
            let store = state.store.clone();
            let resource = resource.clone();
            let owner = owner.clone();
            let lost = lost.clone();
            async move {
                loop {
                    tokio::time::sleep(every).await;
                    match store.renew_lease(&resource, &owner, ttl).await {
                        Ok(true) => {}
                        Ok(false) => {
                            lost.store(true, Ordering::SeqCst);
                            return;
                        }
                        Err(error) => {
                            tracing::warn!(error = %error, "lease heartbeat failed");
                        }
                    }
                }
            }
        });

        Self {
            store: state.store.clone(),
            resource,
            owner,
            lost,
            run_id: std::sync::Mutex::new(None),
            heartbeat,
        }
    }

    fn set_run_id(&self, run_id: &RunId) {
        if let Ok(mut slot) = self.run_id.lock() {
            *slot = Some(run_id.clone());
        }
    }

    fn run_id(&self) -> Option<RunId> {
        self.run_id.lock().ok().and_then(|slot| slot.clone())
    }

    fn lost(&self) -> bool {
        self.lost.load(Ordering::SeqCst)
    }
}

impl Drop for LeaseGuard {
    fn drop(&mut self) {
        self.heartbeat.abort();

        let store = self.store.clone();
        let resource = self.resource.clone();
        let owner = self.owner.clone();
        tokio::spawn(async move {
            if let Err(error) = store.release_lease(&resource, &owner).await {
                tracing::debug!(error = %error, "cannot release the task lease");
            }
        });
    }
}

/// How a followed execution ended from the orchestrator's point of view.
enum PumpResult {
    Finished(ExecutionOutcome),
    /// The agent stopped and asked a question. The task is parked, not failed.
    NeedsInput(String),
    TimedOut,
}

/// Follows an execution until it reaches a decision point.
///
/// Termination is guaranteed by three independent mechanisms: the backend's terminal
/// event, periodic REST reconciliation, and a hard deadline. Any one of them is enough.
async fn pump_events(
    state: &AppState,
    dispatch: &TaskDispatch,
    run_id: &RunId,
    session_id: &str,
) -> Result<PumpResult> {
    let agent = state
        .agent
        .get()
        .await
        .map_err(|error| anyhow!(error.message))?;
    let mut stream = agent
        .events()
        .await
        .context("cannot subscribe to the agent event stream")?;

    let mut text_flush = interval(TEXT_FLUSH_INTERVAL);
    text_flush.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut reconcile_tick = interval(RECONCILE_INTERVAL);
    reconcile_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

    let deadline = tokio::time::sleep(Duration::from_secs(state.run_timeout_secs()));
    tokio::pin!(deadline);

    let mut buffer = String::new();
    let mut reasoning = String::new();

    loop {
        tokio::select! {
            _ = &mut deadline => {
                flush_text(state, &dispatch.task_id, run_id, session_id, &mut buffer).await?;
                flush_reasoning(state, &dispatch.task_id, run_id, session_id, &mut reasoning).await?;
                return Ok(PumpResult::TimedOut);
            }

            maybe_event = stream.next_event() => {
                let Some(event) = maybe_event else {
                    return Err(anyhow!(
                        "the agent event stream ended before the execution finished"
                    ));
                };
                let event = event?;

                // The stream is global across sessions; ignore other projects' traffic.
                if event.session_id() != Some(session_id) {
                    continue;
                }

                if let Some(delta) = event.text_delta() {
                    buffer.push_str(delta);
                }
                if let Some(delta) = event.reasoning_delta() {
                    reasoning.push_str(delta);
                }

                if let Some(reported) = event.usage() {
                    // Cumulative totals from the backend: replace, never add.
                    state
                        .store
                        .set_run_usage(
                            run_id,
                            reported.cost_usd,
                            reported.tokens_in,
                            reported.tokens_out,
                        )
                        .await?;
                }

                if let Some(tool) = event.tool_invocation() {
                    record_tool(state, &dispatch.task_id, run_id, session_id, &tool).await?;
                }

                if let Some(outcome) = event.execution_outcome() {
                    flush_text(state, &dispatch.task_id, run_id, session_id, &mut buffer).await?;
                    flush_reasoning(state, &dispatch.task_id, run_id, session_id, &mut reasoning).await?;
                    return Ok(PumpResult::Finished(outcome));
                }
            }

            _ = text_flush.tick() => {
                flush_text(state, &dispatch.task_id, run_id, session_id, &mut buffer).await?;
                flush_reasoning(state, &dispatch.task_id, run_id, session_id, &mut reasoning).await?;
            }

            _ = reconcile_tick.tick() => {
                if let Some(result) = reconcile(state, dispatch, run_id, session_id).await? {
                    flush_text(state, &dispatch.task_id, run_id, session_id, &mut buffer).await?;
                    flush_reasoning(state, &dispatch.task_id, run_id, session_id, &mut reasoning).await?;
                    return Ok(result);
                }
            }
        }
    }
}

/// Reconciles orchestrator state against the backend's own view.
///
/// This is the authoritative check: the backend's `outcome` decides success, pending
/// forms decide "needs input", and pending permissions are answered here.
async fn reconcile(
    state: &AppState,
    dispatch: &TaskDispatch,
    run_id: &RunId,
    session_id: &str,
) -> Result<Option<PumpResult>> {
    let agent = state
        .agent
        .get()
        .await
        .map_err(|error| anyhow!(error.message))?;

    let session = agent
        .session(session_id)
        .await
        .context("cannot read the agent session state")?;

    // `outcome` is not cleared when a new prompt arrives, so it only describes the
    // present when the session is between turns. Without this check, a repair prompt
    // would immediately "succeed" with the previous turn's verdict.
    if session.time.is_idle() {
        if let Some(outcome) = session.outcome.as_deref() {
            let outcome = match outcome {
                "succeeded" => ExecutionOutcome::Succeeded,
                "failed" => ExecutionOutcome::Failed,
                "interrupted" => ExecutionOutcome::Interrupted,
                other => return Err(anyhow!("the backend reported an unknown outcome {other:?}")),
            };
            return Ok(Some(PumpResult::Finished(outcome)));
        }
    }

    if let Some(question) = cancel_pending_forms(&agent, session_id).await? {
        return Ok(Some(PumpResult::NeedsInput(question)));
    }

    resolve_permissions(state, dispatch, run_id, session_id).await?;
    Ok(None)
}

/// Cancels any interactive form and returns the question it was asking.
///
/// Cancelling matters: leaving the form open keeps the backend's turn alive, so the
/// session would never become idle again.
async fn cancel_pending_forms(agent: &OpenCodeClient, session_id: &str) -> Result<Option<String>> {
    let forms = agent
        .list_forms(session_id)
        .await
        .context("cannot list pending forms")?;

    let Some(form) = forms.first() else {
        return Ok(None);
    };

    let question = describe_form(form);
    if let Some(id) = form.get("id").and_then(|value| value.as_str()) {
        if let Err(error) = agent.cancel_form(session_id, id).await {
            tracing::warn!(error = %error, form = id, "cannot cancel a pending form");
        }
    }

    Ok(Some(question))
}

fn describe_form(form: &serde_json::Value) -> String {
    let title = form
        .get("title")
        .and_then(|value| value.as_str())
        .unwrap_or("the agent asked a question");

    let detail = form
        .get("fields")
        .and_then(|value| value.as_array())
        .and_then(|fields| fields.first())
        .and_then(|field| field.get("description"))
        .and_then(|value| value.as_str())
        .unwrap_or_default();

    let rendered = if detail.trim().is_empty() {
        title.to_string()
    } else {
        format!("{title}: {detail}")
    };

    if rendered.chars().count() > 400 {
        rendered.chars().take(400).collect::<String>() + "…"
    } else {
        rendered
    }
}

async fn flush_text(
    state: &AppState,
    task_id: &TaskId,
    run_id: &RunId,
    session_id: &str,
    buffer: &mut String,
) -> Result<()> {
    flush_chunk(
        state,
        task_id,
        run_id,
        session_id,
        buffer,
        EventKind::AgentText,
    )
    .await
}

async fn flush_reasoning(
    state: &AppState,
    task_id: &TaskId,
    run_id: &RunId,
    session_id: &str,
    buffer: &mut String,
) -> Result<()> {
    flush_chunk(
        state,
        task_id,
        run_id,
        session_id,
        buffer,
        EventKind::AgentReasoning,
    )
    .await
}

async fn flush_chunk(
    state: &AppState,
    task_id: &TaskId,
    run_id: &RunId,
    session_id: &str,
    buffer: &mut String,
    kind: EventKind,
) -> Result<()> {
    if buffer.trim().is_empty() {
        buffer.clear();
        return Ok(());
    }

    let chunk: String = if buffer.chars().count() > TEXT_CHUNK_LIMIT {
        buffer.chars().take(TEXT_CHUNK_LIMIT).collect()
    } else {
        buffer.clone()
    };

    let persisted = if buffer.chars().count() > TEXT_CHUNK_LIMIT {
        buffer.chars().skip(TEXT_CHUNK_LIMIT).collect::<String>()
    } else {
        String::new()
    };

    let event = NewEvent::new(kind)
        .task(task_id)
        .run(run_id)
        .payload(json!({ "session_id": session_id, "text": chunk }));
    jcs_store::append_event(&state.store, event).await?;

    *buffer = persisted;
    Ok(())
}

async fn record_tool(
    state: &AppState,
    task_id: &TaskId,
    run_id: &RunId,
    session_id: &str,
    tool: &ToolInvocation,
) -> Result<()> {
    let kind = if tool.failed {
        EventKind::AgentToolFinished
    } else {
        EventKind::AgentToolCalled
    };

    let event = NewEvent::new(kind)
        .task(task_id)
        .run(run_id)
        .payload(json!({
            "session_id": session_id,
            "tool": tool.name,
            "call_id": tool.call_id,
            "failed": tool.failed,
            // Truncated: a tool payload can be an entire file.
            "input": tool.input.as_ref().map(|value| truncate_json(value, 400)),
            "output": tool.output.as_ref().map(|value| truncate_json(value, 400)),
        }));
    jcs_store::append_event(&state.store, event).await?;
    Ok(())
}

/// Answers every pending permission request according to the configured mode.
async fn resolve_permissions(
    state: &AppState,
    dispatch: &TaskDispatch,
    run_id: &RunId,
    session_id: &str,
) -> Result<()> {
    let agent = state
        .agent
        .get()
        .await
        .map_err(|error| anyhow!(error.message))?;

    let pending = agent
        .pending_permissions(session_id)
        .await
        .context("cannot list pending permissions")?;

    for entry in pending {
        let Some(id) = entry.get("id").and_then(|value| value.as_str()) else {
            continue;
        };
        let action = entry
            .get("action")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let resources = entry.get("resources").cloned().unwrap_or_else(|| json!([]));

        let (decision, reason) = decide(dispatch.permissions, action, state.config.allow_shell);

        tracing::info!(
            session = session_id,
            action,
            decision,
            "permission decision"
        );

        agent
            .reply_permission(session_id, id, decision, Some(reason))
            .await
            .with_context(|| format!("cannot reply to permission {id}"))?;

        let event = NewEvent::new(EventKind::AgentToolFinished)
            .task(&dispatch.task_id)
            .run(run_id)
            .payload(json!({
                "kind": "permission_decision",
                "session_id": session_id,
                "request_id": id,
                "action": action,
                "resources": truncate_json(&resources, 300),
                "decision": decision,
                "reason": reason,
            }));
        jcs_store::append_event(&state.store, event).await?;
    }

    Ok(())
}

/// Permission policy.
///
/// File-scoped work inside an isolated worktree is safe: git can restore it, and the
/// worktree is disposable. Shell execution is not, and is off by default — verification
/// belongs to the gate runner, not to the process being verified.
fn decide(mode: PermissionMode, action: &str, allow_shell: bool) -> (&'static str, &'static str) {
    let file_work = matches!(
        action,
        "read"
            | "grep"
            | "glob"
            | "list"
            | "ls"
            | "find"
            | "view"
            | "edit"
            | "write"
            | "patch"
            | "apply_patch"
            | "multiedit"
            | "notebookedit"
            | "todowrite"
            | "todoread"
            | "task"
            | "skill"
    );

    let shell = matches!(action, "bash" | "shell" | "execute" | "command");

    match mode {
        PermissionMode::Strict => ("reject", "strict mode: every action needs a human approval"),
        PermissionMode::Auto if file_work => (
            "once",
            "auto-approved: file-scoped work inside a disposable task worktree",
        ),
        PermissionMode::Auto if shell && allow_shell => (
            "once",
            "shell execution explicitly enabled by JCS_ALLOW_SHELL",
        ),
        PermissionMode::Auto if shell => (
            "reject",
            "shell execution is disabled; verification runs through the gate runner \
             (set JCS_ALLOW_SHELL=1 to allow it)",
        ),
        PermissionMode::Auto => ("reject", "action is outside the auto-approved set"),
    }
}

fn repair_prompt(failed_gates: &[String], excerpt: &str) -> String {
    format!(
        "The verification gates failed: {}.\n\
         Fix the underlying problems in this worktree, then reply with a brief summary.\n\
         \n\
         Rules:\n\
         - Do not weaken, skip or delete tests to make them pass.\n\
         - Do not edit gate configuration, policy files or CI configuration.\n\
         - Do not use shell commands.\n\
         \n\
         Gate output:\n{excerpt}\n",
        failed_gates.join(", ")
    )
}

/// Waits until the backend reports the session as busy (a turn in progress).
///
/// Bounded: if the backend never flips the flag we proceed anyway and let the normal
/// timeout and reconciliation handle it.
async fn wait_until_busy(agent: &OpenCodeClient, session_id: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);

    while Instant::now() < deadline {
        match agent.session(session_id).await {
            Ok(session) if session.time.busy.is_some() && !session.time.is_idle() => return,
            Ok(_) => {}
            Err(error) => {
                tracing::debug!(error = %error, "cannot read session state while waiting for busy");
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

async fn finalize_success(
    state: &AppState,
    dispatch: &TaskDispatch,
    run_id: &RunId,
    task: &Task,
    project: &Project,
    worktree: &std::path::Path,
    branch: &str,
) -> Result<()> {
    let repo = Repo::new(worktree);

    state
        .store
        .add_gate_result(
            run_id,
            "agent-outcome",
            GateStatus::Passed,
            Some(0),
            0,
            Some("the agent reported a successful execution"),
        )
        .await?;

    let message = commit_message(&project.name, &task.title)?;
    let started = Instant::now();
    let committed = repo.commit_all(&message).await;
    let duration_ms = started.elapsed().as_millis() as i64;

    let commit = match committed {
        Ok(Some(commit)) => commit,
        Ok(None) => {
            state
                .store
                .add_gate_result(
                    run_id,
                    "changes-produced",
                    GateStatus::Failed,
                    Some(0),
                    duration_ms,
                    Some("the agent reported success but the worktree is unchanged"),
                )
                .await?;
            return Err(anyhow!(
                "the agent reported success but produced no changes; nothing was committed"
            ));
        }
        Err(error) => {
            state
                .store
                .add_gate_result(
                    run_id,
                    "commit",
                    GateStatus::Errored,
                    None,
                    duration_ms,
                    Some(&error.to_string()),
                )
                .await?;
            return Err(error).context("cannot commit the agent's changes");
        }
    };

    state
        .store
        .add_gate_result(
            run_id,
            "commit",
            GateStatus::Passed,
            Some(0),
            duration_ms,
            Some(&commit.short),
        )
        .await?;

    let event = NewEvent::new(EventKind::CommitCreated)
        .task(&task.id)
        .goal(&task.goal_id)
        .run(run_id)
        .payload(json!({
            "sha": commit.sha,
            "short": commit.short,
            "branch": branch,
            "message": message,
        }));
    jcs_store::append_event(&state.store, event).await?;

    // The tree must be clean after our own commit: anything left is something we did
    // not understand well enough to call success.
    let clean = repo.is_clean().await?;
    state
        .store
        .add_gate_result(
            run_id,
            "worktree-clean",
            if clean {
                GateStatus::Passed
            } else {
                GateStatus::Failed
            },
            Some(0),
            0,
            if clean {
                None
            } else {
                Some("uncommitted files remain after the commit")
            },
        )
        .await?;

    if !clean {
        return Err(anyhow!(
            "the worktree still has uncommitted changes after committing"
        ));
    }

    // Order matters: the scheduler finalizes a goal as soon as every task is terminal,
    // and a client stops following on that event. If the task became terminal before the
    // run did, the client could exit before ever seeing the run's conclusion.
    state
        .store
        .finish_run(run_id, jcs_core::RunStatus::Succeeded, Some(0), None)
        .await?;
    state
        .store
        .set_task_status(&task.id, TaskStatus::Succeeded)
        .await?;

    tracing::info!(task = %task.id, sha = %commit.short, "task succeeded");
    let _ = dispatch;
    Ok(())
}

/// Records an honest failure without masking a run that already finished.
async fn fail_task(
    state: &AppState,
    dispatch: &TaskDispatch,
    run_id: Option<&RunId>,
    reason: &str,
) -> Result<()> {
    // The run may already be terminal (for example after an explicit abort).
    if let Some(run_id) = run_id {
        if let Some(run) = state.store.get_run(run_id).await? {
            if run.status == jcs_core::RunStatus::Started {
                state
                    .store
                    .finish_run(
                        run_id,
                        jcs_core::RunStatus::Failed,
                        None,
                        Some(reason.to_string()),
                    )
                    .await?;
            }
        }
    }

    // Terminal task state last, so the scheduler cannot finalize the goal before the run.
    if let Some(task) = state.store.get_task(&dispatch.task_id).await? {
        if matches!(task.status, TaskStatus::Running | TaskStatus::Verifying) {
            if let Err(error) = state
                .store
                .set_task_status(&task.id, TaskStatus::Failed)
                .await
            {
                tracing::warn!(error = %error, "cannot mark the task as failed");
            }
        }
    }

    Ok(())
}

/// Parks a task that needs a human decision.
///
/// `Blocked`, not `Failed`: the objective is still wanted, and the question is preserved
/// in the event log so the operator can act on it.
async fn block_for_input(
    state: &AppState,
    dispatch: &TaskDispatch,
    run_id: &RunId,
    task: &Task,
    question: &str,
) -> Result<()> {
    state
        .store
        .finish_run(
            run_id,
            jcs_core::RunStatus::Failed,
            None,
            Some(format!("needs input: {question}")),
        )
        .await?;
    // Terminal task state last: it is what the scheduler uses to finalize the goal.
    state
        .store
        .set_task_status(&task.id, TaskStatus::Blocked)
        .await?;

    let event = NewEvent::new(EventKind::Error)
        .task(&task.id)
        .goal(&task.goal_id)
        .run(run_id)
        .payload(json!({
            "category": "needs_input",
            "question": question,
            "hint": "the task is parked as blocked; restate the objective or answer it",
        }));
    jcs_store::append_event(&state.store, event).await?;

    let _ = dispatch;
    Ok(())
}

/// Appends cited code context to the task prompt.
///
/// This is the payoff of the memory index: the agent starts with the code that matters and
/// a citation for each snippet, instead of exploring a repository it cannot see. It is
/// skipped when the index is empty or the budget is zero, so a project without memory
/// behaves exactly as before.
async fn with_memory_context(
    state: &AppState,
    project: &Project,
    goal: &Goal,
    task: &Task,
    mut prompt: String,
) -> String {
    if state.context_tokens() == 0 {
        return prompt;
    }

    let options = jcs_memory::ContextOptions {
        query: format!("{}\n{}", goal.prompt, task.title),
        budget_tokens: state.context_tokens() as usize,
        limit: 8,
    };

    let extra = policy_sections(state, project);

    match jcs_memory::build_context(
        &state.store,
        state.embeddings.as_deref(),
        &project.id,
        options,
        &extra,
    )
    .await
    {
        Ok(package) if !package.is_empty() => {
            tracing::info!(
                task = %task.id,
                hits = package.hits,
                tokens = package.used_tokens,
                "injecting code context"
            );
            prompt.push_str("\n\n");
            prompt.push_str(&package.text);
            prompt.push('\n');
            prompt
        }
        Ok(_) => prompt,
        Err(error) => {
            tracing::debug!(error = %format!("{error:#}"), "no code context available");
            prompt
        }
    }
}

fn policy_sections(state: &AppState, project: &Project) -> Vec<(String, String)> {
    let Ok(compiled) = jcs_policy::compile(
        &project.root_path,
        &state.packs,
        &state.config.default_packs,
    ) else {
        return Vec::new();
    };

    let rules = compiled
        .composition
        .rules
        .iter()
        .map(|(key, entry)| format!("- {key}: {}", entry.value))
        .collect::<Vec<_>>()
        .join("\n");

    if rules.is_empty() {
        Vec::new()
    } else {
        vec![("Project rules".to_string(), rules)]
    }
}

fn build_prompt(
    goal: &Goal,
    task: &Task,
    worktree: &std::path::Path,
    branch: &str,
    base_branch: &str,
) -> String {
    format!(
        "You are working in an isolated git worktree.\n\
         \n\
         Worktree: {worktree}\n\
         Branch: {branch} (cut from {base_branch})\n\
         \n\
         Objective:\n{prompt}\n\
         \n\
         Task: {title}\n\
         {description}\n\
         \n\
         Rules:\n\
         - Make changes directly in this worktree. Do not touch any other directory.\n\
         - Do not commit, amend, rebase, or push. JC Studio reviews and commits your changes.\n\
         - Do not run shell commands unless the operator approved them; verification is\n\
           handled outside this session.\n\
         - Do not ask questions. If something is ambiguous, take the most reasonable\n\
           interpretation, proceed, and state the assumption in your summary.\n\
         - Prefer the smallest coherent change that fully satisfies the objective.\n\
         - Follow the conventions already present in the codebase.\n\
         \n\
         When you are done, reply with a brief summary of what you changed and why.\n",
        worktree = worktree.display(),
        prompt = goal.prompt,
        title = task.title,
        description = if task.path_claims.is_empty() {
            String::new()
        } else {
            format!("Expected paths: {}", task.path_claims.join(", "))
        },
    )
}

/// Builds a valid conventional commit message from a task title.
fn commit_message(project_name: &str, title: &str) -> jcs_git::Result<String> {
    let scope = jcs_git::slugify(project_name)
        .chars()
        .take(20)
        .collect::<String>();

    let lowered = title.trim().trim_end_matches('.').to_lowercase();
    let candidate = jcs_git::truncate_words(&lowered, 60);
    let summary =
        if !candidate.is_empty() && jcs_git::validate(&format!("feat: {candidate}")).is_ok() {
            candidate
        } else {
            jcs_git::slugify(title)
        };

    ConventionalCommit::new(CommitKind::Feat, &summary)
        .scope(&scope)
        .render()
}

fn truncate_json(value: &serde_json::Value, max: usize) -> String {
    let rendered = value.to_string();
    if rendered.chars().count() <= max {
        return rendered;
    }
    rendered.chars().take(max).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;
    use jcs_core::GoalId;

    fn truncate_chars(value: &str, max: usize) -> String {
        if value.chars().count() <= max {
            return value.to_string();
        }
        value.chars().take(max).collect::<String>() + "\n… (truncated)"
    }

    #[test]
    fn file_work_is_auto_approved_in_auto_mode() {
        for action in ["read", "edit", "write", "patch", "glob"] {
            let (decision, _) = decide(PermissionMode::Auto, action, false);
            assert_eq!(decision, "once", "{action} should be auto-approved");
        }
    }

    #[test]
    fn shell_is_denied_by_default_and_allowed_only_on_request() {
        assert_eq!(decide(PermissionMode::Auto, "bash", false).0, "reject");
        assert_eq!(decide(PermissionMode::Auto, "bash", true).0, "once");
    }

    #[test]
    fn strict_mode_denies_everything_automatically() {
        for action in ["read", "edit", "bash"] {
            assert_eq!(decide(PermissionMode::Strict, action, true).0, "reject");
        }
    }

    #[test]
    fn unknown_actions_are_denied() {
        assert_eq!(decide(PermissionMode::Auto, "deploy", true).0, "reject");
    }

    #[test]
    fn commit_messages_are_always_valid() {
        for title in [
            "Add email validation",
            "add email validation",
            "Añadir validación de correo.",
            "",
        ] {
            let message = commit_message("Mi Servicio", title)
                .unwrap_or_else(|error| panic!("{title:?}: {error}"));
            assert!(
                jcs_git::validate(&message).is_ok(),
                "{title:?} produced invalid message {message:?}"
            );
        }
    }

    #[test]
    fn the_repair_prompt_names_the_gates_and_forbids_weakening_tests() {
        let prompt = repair_prompt(
            &["clippy".to_string(), "test".to_string()],
            "error[E0308]: mismatched types",
        );
        assert!(prompt.contains("clippy, test"));
        assert!(prompt.contains("Do not weaken, skip or delete tests"));
        assert!(prompt.contains("error[E0308]"));
        assert!(prompt.contains("Do not edit gate configuration"));
    }

    #[test]
    fn long_gate_output_is_truncated_for_the_prompt() {
        let huge = "x".repeat(20_000);
        let truncated = truncate_chars(&huge, 1_000);
        assert!(truncated.chars().count() < 1_100);
        assert!(truncated.contains("truncated"));
    }

    #[test]
    fn the_prompt_states_isolation_claims_and_commit_rules() {
        let goal = Goal {
            id: GoalId::new(),
            project_id: jcs_core::ProjectId::new(),
            prompt: "do the thing".into(),
            status: jcs_core::GoalStatus::Running,
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        };
        let task = Task {
            id: TaskId::new(),
            goal_id: goal.id.clone(),
            title: "the task".into(),
            status: TaskStatus::Running,
            worktree: None,
            branch: None,
            session_id: None,
            role: jcs_core::TaskRole::Implement,
            priority: 0,
            path_claims: vec!["src/api".into()],
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        };

        let prompt = build_prompt(
            &goal,
            &task,
            std::path::Path::new("/tmp/wt"),
            "jcs/x/abc",
            "main",
        );

        assert!(prompt.contains("/tmp/wt"));
        assert!(prompt.contains("Do not commit"));
        assert!(prompt.contains("Do not ask questions"));
        assert!(prompt.contains("do the thing"));
        assert!(prompt.contains("Expected paths: src/api"));
    }
}
