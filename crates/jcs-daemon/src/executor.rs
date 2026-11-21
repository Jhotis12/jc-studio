//! Task execution pipeline.
//!
//! One task = one isolated worktree + one agent session + one commit, gated.
//!
//! Ordering matters and is deliberate:
//!   1. the worktree exists before the agent does, so the agent cannot escape it;
//!   2. the task reaches `Verifying` before any gate runs, so success is never recorded
//!      on the agent's word alone;
//!   3. the run only finishes after the gates have their verdict.

use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use jcs_agent::{ExecutionOutcome, ModelRef, OpenCodeClient, ToolInvocation};
use jcs_core::{
    EventKind, GateStatus, Goal, GoalStatus, NewEvent, Project, RunId, RunStatus, Task, TaskId,
    TaskStatus,
};
use jcs_git::{branch_name, CommitKind, ConventionalCommit, Repo};
use jcs_proto::PermissionMode;
use serde_json::json;
use tokio::time::{interval, MissedTickBehavior};

use crate::state::AppState;

/// How long text deltas are buffered before being persisted as one event.
const TEXT_FLUSH_INTERVAL: Duration = Duration::from_millis(300);
/// How often the backend is reconciled over REST (outcome, forms, permissions).
///
/// Event delivery is an optimization; this poll is what guarantees termination even if
/// an event shape changes or a notification is missed.
const RECONCILE_INTERVAL: Duration = Duration::from_secs(2);
/// Upper bound on a single persisted text chunk.
const TEXT_CHUNK_LIMIT: usize = 4_000;

#[derive(Debug, Clone)]
pub struct TaskRequest {
    pub task_id: TaskId,
    pub run_id: RunId,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub permissions: PermissionMode,
}

/// Starts a task in the background, bounded by the concurrency semaphore.
pub fn spawn(state: AppState, request: TaskRequest) {
    tokio::spawn(async move {
        let permit = match state.permits.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => {
                tracing::error!("executor semaphore closed; task not started");
                return;
            }
        };

        if let Err(error) = run_task(&state, &request).await {
            tracing::error!(
                task = %request.task_id,
                run = %request.run_id,
                error = %error,
                "task failed"
            );
            if let Err(mark) = fail_task(&state, &request, &format!("{error:#}")).await {
                tracing::error!(error = %mark, "could not record the failure");
            }
        }

        drop(permit);
    });
}

async fn run_task(state: &AppState, request: &TaskRequest) -> Result<()> {
    let task = state
        .store
        .get_task(&request.task_id)
        .await?
        .ok_or_else(|| anyhow!("task {} no longer exists", request.task_id))?;
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

    if goal.status == GoalStatus::Pending {
        state
            .store
            .set_goal_status(&goal.id, GoalStatus::Running)
            .await?;
    }

    // 1. Isolate before anything else can touch the repository.
    let worktree = state
        .config
        .worktrees_dir
        .join(project.id.as_str())
        .join(task.id.as_str());
    let branch = branch_name(&goal.prompt, task.id.as_str());

    if !worktree.exists() {
        let repo = Repo::new(&project.root_path);
        repo.require_repository().await.with_context(|| {
            format!(
                "{} is not usable as a project (needs a git repo with at least one commit)",
                project.root_path.display()
            )
        })?;
        repo.create_worktree(&worktree, &branch, &project.base_branch)
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

    let model = match request
        .model
        .clone()
        .or_else(|| state.config.default_model.clone())
    {
        Some(reference) => Some(ModelRef::parse(&reference)?),
        None => None,
    };
    let agent_name = request
        .agent
        .clone()
        .or_else(|| state.config.default_agent.clone());

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

    let prompt = build_prompt(&goal, &worktree, &branch, &project.base_branch);
    agent
        .prompt(&session.id, &prompt, "queue")
        .await
        .context("cannot deliver the initial prompt")?;

    // 3. Follow the execution until the backend states an outcome, the agent asks a
    //    question, or the run exceeds its budget.
    match pump_events(state, request, &session.id).await? {
        PumpResult::Finished(ExecutionOutcome::Succeeded) => {
            state
                .store
                .set_task_status(&task.id, TaskStatus::Verifying)
                .await?;
            finalize_success(state, request, &task, &goal, &project, &worktree, &branch).await
        }
        PumpResult::Finished(ExecutionOutcome::Failed) => {
            Err(anyhow!("the agent reported that the execution failed"))
        }
        PumpResult::Finished(ExecutionOutcome::Interrupted) => {
            Err(anyhow!("the execution was interrupted before it completed"))
        }
        // A question is a human decision, not a hang: park the task and release the run.
        PumpResult::NeedsInput(question) => {
            block_for_input(state, request, &task, &goal, &question).await
        }
        PumpResult::TimedOut => Err(anyhow!(
            "the run exceeded its {}s budget and was stopped",
            state.config.run_timeout_secs
        )),
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
    request: &TaskRequest,
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

    let deadline = tokio::time::sleep(Duration::from_secs(state.config.run_timeout_secs));
    tokio::pin!(deadline);

    let mut buffer = String::new();

    loop {
        tokio::select! {
            _ = &mut deadline => {
                flush_text(state, request, session_id, &mut buffer).await?;
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

                if let Some(reported) = event.usage() {
                    // Cumulative totals from the backend: replace, never add.
                    state
                        .store
                        .set_run_usage(
                            &request.run_id,
                            reported.cost_usd,
                            reported.tokens_in,
                            reported.tokens_out,
                        )
                        .await?;
                }

                if let Some(tool) = event.tool_invocation() {
                    record_tool(state, request, session_id, &tool).await?;
                }

                if let Some(outcome) = event.execution_outcome() {
                    flush_text(state, request, session_id, &mut buffer).await?;
                    return Ok(PumpResult::Finished(outcome));
                }
            }

            _ = text_flush.tick() => {
                flush_text(state, request, session_id, &mut buffer).await?;
            }

            _ = reconcile_tick.tick() => {
                if let Some(result) = reconcile(state, request, session_id).await? {
                    flush_text(state, request, session_id, &mut buffer).await?;
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
    request: &TaskRequest,
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

    if let Some(outcome) = session.outcome.as_deref() {
        let outcome = match outcome {
            "succeeded" => ExecutionOutcome::Succeeded,
            "failed" => ExecutionOutcome::Failed,
            "interrupted" => ExecutionOutcome::Interrupted,
            other => return Err(anyhow!("the backend reported an unknown outcome {other:?}")),
        };
        return Ok(Some(PumpResult::Finished(outcome)));
    }

    if let Some(question) = cancel_pending_forms(&agent, session_id).await? {
        return Ok(Some(PumpResult::NeedsInput(question)));
    }

    resolve_permissions(state, request, session_id).await?;
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
    request: &TaskRequest,
    session_id: &str,
    buffer: &mut String,
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

    let event = NewEvent::new(EventKind::AgentText)
        .task(&request.task_id)
        .run(&request.run_id)
        .payload(json!({ "session_id": session_id, "text": chunk }));
    jcs_store::append_event(&state.store, event).await?;

    *buffer = persisted;
    Ok(())
}

async fn record_tool(
    state: &AppState,
    request: &TaskRequest,
    session_id: &str,
    tool: &ToolInvocation,
) -> Result<()> {
    let kind = if tool.failed {
        EventKind::AgentToolFinished
    } else {
        EventKind::AgentToolCalled
    };

    let event = NewEvent::new(kind)
        .task(&request.task_id)
        .run(&request.run_id)
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
    request: &TaskRequest,
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

        let (decision, reason) = decide(request.permissions, action, state.config.allow_shell);

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
            .task(&request.task_id)
            .run(&request.run_id)
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

/// Parks a task that needs a human decision.
///
/// `Blocked`, not `Failed`: the objective is still wanted, and the question is preserved
/// in the event log so the operator can act on it.
async fn block_for_input(
    state: &AppState,
    request: &TaskRequest,
    task: &Task,
    goal: &Goal,
    question: &str,
) -> Result<()> {
    state
        .store
        .set_task_status(&task.id, TaskStatus::Blocked)
        .await?;
    state
        .store
        .set_goal_status(&goal.id, GoalStatus::Blocked)
        .await?;
    state
        .store
        .finish_run(
            &request.run_id,
            RunStatus::Failed,
            None,
            Some(format!("needs input: {question}")),
        )
        .await?;

    let event = NewEvent::new(EventKind::Error)
        .task(&task.id)
        .goal(&goal.id)
        .run(&request.run_id)
        .payload(json!({
            "category": "needs_input",
            "question": question,
            "hint": "the task is parked as blocked; restate the objective or answer it",
        }));
    jcs_store::append_event(&state.store, event).await?;

    Ok(())
}

async fn finalize_success(
    state: &AppState,
    request: &TaskRequest,
    task: &Task,
    goal: &Goal,
    project: &Project,
    worktree: &std::path::Path,
    branch: &str,
) -> Result<()> {
    let repo = Repo::new(worktree);

    state
        .store
        .add_gate_result(
            &request.run_id,
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
                    &request.run_id,
                    "changes-produced",
                    GateStatus::Failed,
                    Some(0),
                    duration_ms,
                    Some("the agent reported success but the worktree is unchanged"),
                )
                .await?;
            return Err(anyhow!(
                "the agent reported success but produced no changes; \
                 nothing was committed"
            ));
        }
        Err(error) => {
            state
                .store
                .add_gate_result(
                    &request.run_id,
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
            &request.run_id,
            "commit",
            GateStatus::Passed,
            Some(0),
            duration_ms,
            Some(&commit.short),
        )
        .await?;

    let event = NewEvent::new(EventKind::CommitCreated)
        .task(&task.id)
        .goal(&goal.id)
        .run(&request.run_id)
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
            &request.run_id,
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

    state
        .store
        .set_task_status(&task.id, TaskStatus::Succeeded)
        .await?;
    state
        .store
        .set_goal_status(&goal.id, GoalStatus::Succeeded)
        .await?;
    state
        .store
        .finish_run(&request.run_id, RunStatus::Succeeded, Some(0), None)
        .await?;

    Ok(())
}

/// Records an honest failure without masking a run that already finished.
async fn fail_task(state: &AppState, request: &TaskRequest, reason: &str) -> Result<()> {
    if let Some(task) = state.store.get_task(&request.task_id).await? {
        if matches!(task.status, TaskStatus::Running | TaskStatus::Verifying) {
            if let Err(error) = state
                .store
                .set_task_status(&task.id, TaskStatus::Failed)
                .await
            {
                tracing::warn!(error = %error, "cannot mark the task as failed");
            }
        }

        if let Some(goal) = state.store.get_goal(&task.goal_id).await? {
            if goal.status == GoalStatus::Running {
                if let Err(error) = state
                    .store
                    .set_goal_status(&goal.id, GoalStatus::Failed)
                    .await
                {
                    tracing::warn!(error = %error, "cannot mark the goal as failed");
                }
            }
        }
    }

    // The run may already be terminal (for example after an explicit abort).
    if let Some(run) = state.store.get_run(&request.run_id).await? {
        if run.status == RunStatus::Started {
            state
                .store
                .finish_run(
                    &request.run_id,
                    RunStatus::Failed,
                    None,
                    Some(reason.to_string()),
                )
                .await?;
        }
    }

    Ok(())
}

fn build_prompt(
    goal: &Goal,
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
    fn prompt_states_the_isolation_and_the_commit_rules() {
        let goal = Goal {
            id: GoalId::new(),
            project_id: jcs_core::ProjectId::new(),
            prompt: "do the thing".into(),
            status: GoalStatus::Running,
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        };
        let prompt = build_prompt(&goal, std::path::Path::new("/tmp/wt"), "jcs/x/abc", "main");

        assert!(prompt.contains("/tmp/wt"));
        assert!(prompt.contains("Do not commit"));
        assert!(prompt.contains("Do not ask questions"));
        assert!(prompt.contains("do the thing"));
        // The duplicated, truncated title must not appear: it read as a contradiction.
        assert!(!prompt.contains("Task:"));
    }
}
