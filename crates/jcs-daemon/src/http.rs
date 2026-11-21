//! HTTP surface of the daemon, served over a unix domain socket.
//!
//! JSON for commands and queries, newline-delimited JSON for the event stream (ADR 0006).
//! The socket's `0600` permissions are the security boundary: it is a single-user daemon,
//! so there is no token to leak. A protocol header guards against version skew.

use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use bytes::Bytes;
use futures_util::stream;
use jcs_core::{
    EventKind, Goal, GoalStatus, NewEvent, Project, ProjectId, RunStatus, StatusMachine, TaskId,
    TaskStatus,
};
use jcs_proto::{
    AddProjectRequest, AskRequest, AskResponse, ErrorBody, GateOutcomeView, GateRunRequest,
    GateRunResponse, GateView, GoalView, HealthResponse, McpAddRequest, McpServerEntry,
    ModelCatalogEntry, OrchestratorChatRequest, OrchestratorMessage, PackCatalogEntry,
    PolicyGateView, PolicyPackView, PolicyRuleView, PolicyView, ProjectListResponse,
    ProjectResponse, ProjectView, RenderPolicyRequest, RenderPolicyResponse, RenderedFileView,
    RunDetailResponse, RunView, SettingsPatch, SettingsView, SkillCatalogEntry, StatusResponse,
    TaskActionResponse, TaskIterateRequest, TaskView, Totals, PROTOCOL_HEADER, PROTOCOL_VERSION,
};
use serde::Deserialize;
use serde_json::json;

use crate::error::{ApiError, ApiResult};

use crate::state::AppState;

const LIST_LIMIT: i64 = 50;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/projects", get(list_projects).post(add_project))
        .route("/projects/{id}", delete(remove_project).put(update_project))
        .route("/goals", get(list_goals))
        .route("/tasks", get(list_tasks))
        .route("/tasks/{id}/events", get(task_events))
        .route("/tasks/{id}/iterate", post(iterate_task))
        .route("/tasks/{id}/merge-retry", post(retry_task_merge))
        .route("/runs", get(list_runs))
        .route("/runs/{id}", get(run_detail))
        .route("/ask", post(ask))
        .route("/abort", post(abort))
        .route("/events", get(events))
        .route("/events/history", get(event_history))
        .route("/policy", get(policy))
        .route("/policy/packs", get(policy_packs))
        .route("/policy/render", post(render_policy))
        .route("/gates", get(gates))
        .route("/gates/run", post(run_gates))
        .route("/metrics", get(metrics))
        .route("/settings", get(settings).put(update_settings))
        .route(
            "/orchestrator",
            get(orchestrator_history)
                .post(orchestrator_chat)
                .delete(orchestrator_reset),
        )
        .route("/catalog/models", get(catalog_models))
        .route("/catalog/skills", get(catalog_skills))
        .route("/catalog/mcp", get(catalog_mcp).post(add_mcp_server))
        .route("/catalog/mcp/{server}", delete(remove_mcp_server))
        .route("/catalog/mcp/{server}/connect", post(connect_mcp_server))
        .route(
            "/catalog/mcp/{server}/disconnect",
            post(disconnect_mcp_server),
        )
        .route("/memory/index", post(memory_index))
        .route("/memory/search", get(memory_search))
        .route("/memory/stats", get(memory_stats))
        .route("/goals/{id}", get(goal_detail).delete(delete_goal))
        .route("/goals/{id}/resume", post(resume_goal))
        .route("/goals/{id}/cancel", post(cancel_goal))
        .layer(middleware::from_fn(protocol_guard))
        .with_state(state)
}

/// Refuses a client whose protocol version differs, when it declares one.
///
/// A missing header is accepted so `curl` remains a usable debugging tool on a
/// `0600` socket.
async fn protocol_guard(request: Request, next: Next) -> Response {
    if let Some(value) = request.headers().get(PROTOCOL_HEADER) {
        let declared = value.to_str().unwrap_or_default();
        if declared != PROTOCOL_VERSION.to_string() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorBody::new(
                    "protocol_mismatch",
                    format!(
                        "client speaks protocol {declared}, daemon speaks {PROTOCOL_VERSION}; \
                         rebuild the client"
                    ),
                )),
            )
                .into_response();
        }
    }
    next.run(request).await
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let agent = match tokio::time::timeout(Duration::from_secs(3), state.agent.get()).await {
        Ok(Ok(client)) => match client.health().await {
            Ok(info) => jcs_proto::AgentHealth {
                backend: "opencode".to_string(),
                reachable: true,
                base_url: Some(client.endpoint().base_url().to_string()),
                version: Some(info.version),
                message: None,
            },
            Err(error) => jcs_proto::AgentHealth {
                backend: "opencode".to_string(),
                reachable: false,
                base_url: Some(client.endpoint().base_url().to_string()),
                version: None,
                message: Some(error.to_string()),
            },
        },
        Ok(Err(error)) => jcs_proto::AgentHealth {
            backend: "opencode".to_string(),
            reachable: false,
            base_url: None,
            version: None,
            message: Some(error.message),
        },
        Err(_) => jcs_proto::AgentHealth {
            backend: "opencode".to_string(),
            reachable: false,
            base_url: None,
            version: None,
            message: Some("timed out discovering the agent backend".to_string()),
        },
    };

    Json(HealthResponse {
        ok: true,
        version: env!("CARGO_PKG_VERSION").to_string(),
        protocol: PROTOCOL_VERSION,
        uptime_secs: state.uptime_secs(),
        database: state.config.db_path.display().to_string(),
        agent,
        started_at_ms: state.started_at_ms,
    })
}

async fn status(State(state): State<AppState>) -> ApiResult<Json<StatusResponse>> {
    let projects = state.store.list_projects().await?;
    let goals = state.store.list_goals(LIST_LIMIT).await?;
    let tasks = state.store.list_tasks(LIST_LIMIT).await?;
    let runs = state.store.list_runs(LIST_LIMIT).await?;
    let cost = state.store.total_cost_usd(None).await?;

    // Counts come from SQL, not from the bounded lists above: a dashboard that shows
    // "50 tasks" when there are 400 is worse than showing nothing.
    let totals = Totals {
        projects: state.store.project_count().await?,
        goals: state.store.goal_count().await?,
        tasks: state.store.task_count().await?,
        cost_usd: cost,
        // Token totals are per listed run; the cost total is authoritative.
        tokens_in: runs.iter().map(|run| run.tokens_in).sum(),
        tokens_out: runs.iter().map(|run| run.tokens_out).sum(),
    };

    let health = health(State(state.clone())).await.0;

    Ok(Json(StatusResponse {
        health,
        projects: projects.iter().map(project_view).collect(),
        goals: goals.iter().map(goal_view).collect(),
        tasks: task_views(&state, &tasks).await?,
        runs: runs.iter().map(run_view).collect(),
        totals,
    }))
}

async fn list_projects(State(state): State<AppState>) -> ApiResult<Json<ProjectListResponse>> {
    let projects = state.store.list_projects().await?;
    Ok(Json(ProjectListResponse {
        projects: projects.iter().map(project_view).collect(),
    }))
}

async fn add_project(
    State(state): State<AppState>,
    Json(request): Json<AddProjectRequest>,
) -> ApiResult<Json<ProjectResponse>> {
    if request.path.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_path",
            "path is empty; pass the absolute path of a git repository",
        ));
    }

    // The client resolves `.`/`~` against *its* working directory; the daemon must not
    // guess, because it usually runs from `$HOME` under a service manager.
    let requested = std::path::PathBuf::from(&request.path);
    if !requested.is_absolute() {
        return Err(ApiError::bad_request(
            "relative_path",
            format!(
                "{:?} is not absolute; the client must resolve it before sending",
                request.path
            ),
        ));
    }

    let root = std::fs::canonicalize(&requested).map_err(|error| {
        ApiError::bad_request(
            "invalid_path",
            format!("cannot resolve {}: {error}", requested.display()),
        )
    })?;

    let repo = jcs_git::Repo::new(&root);
    repo.require_repository().await?;

    let base_branch = match request.base_branch {
        Some(branch) => {
            if !repo.branch_exists(&branch).await? {
                return Err(ApiError::bad_request(
                    "unknown_branch",
                    format!("branch {branch:?} does not exist in {}", root.display()),
                ));
            }
            branch
        }
        None => repo.current_branch().await?,
    };

    let name = request
        .name
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            root.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "project".to_string())
        });

    let project = Project {
        id: ProjectId::new(),
        name,
        root_path: root,
        base_branch,
        created_at: jiff::Timestamp::now(),
        auto_merge: false,
        budget_usd_per_day: None,
        budget_tokens_per_day: None,
    };

    state.store.insert_project(&project).await?;

    // Index in the background: adding a project should not wait for a disk sweep.
    crate::memory_service::spawn_index(state.clone(), project.clone());

    Ok(Json(ProjectResponse {
        project: project_view(&project),
    }))
}

async fn update_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(settings): Json<jcs_proto::ProjectSettings>,
) -> ApiResult<Json<ProjectResponse>> {
    let id = ProjectId::parse(id)?;

    if let Some(budget) = settings.budget_usd_per_day {
        if budget < 0.0 || !budget.is_finite() {
            return Err(ApiError::bad_request(
                "invalid_budget",
                "budget_usd_per_day must be a finite, non-negative number",
            ));
        }
    }
    if let Some(tokens) = settings.budget_tokens_per_day {
        if tokens < 0 {
            return Err(ApiError::bad_request(
                "invalid_budget",
                "budget_tokens_per_day must be non-negative",
            ));
        }
    }

    state
        .store
        .update_project_settings(
            &id,
            settings.auto_merge,
            settings.budget_usd_per_day,
            settings.budget_tokens_per_day,
        )
        .await?;

    let project = state
        .store
        .get_project(&id)
        .await?
        .ok_or_else(|| ApiError::not_found("project_not_found", format!("no project {id}")))?;

    Ok(Json(ProjectResponse {
        project: project_view(&project),
    }))
}

async fn remove_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<jcs_proto::OkResponse>> {
    let id = ProjectId::parse(id)?;
    state.store.delete_project(&id).await?;
    Ok(Json(jcs_proto::OkResponse { ok: true }))
}

async fn list_goals(State(state): State<AppState>) -> ApiResult<Json<Vec<GoalView>>> {
    let goals = state.store.list_goals(LIST_LIMIT).await?;
    Ok(Json(goals.iter().map(goal_view).collect()))
}

async fn list_tasks(State(state): State<AppState>) -> ApiResult<Json<Vec<TaskView>>> {
    let tasks = state.store.list_tasks(LIST_LIMIT).await?;
    Ok(Json(tasks.iter().map(task_view).collect()))
}

async fn list_runs(State(state): State<AppState>) -> ApiResult<Json<Vec<RunView>>> {
    let runs = state.store.list_runs(LIST_LIMIT).await?;
    Ok(Json(runs.iter().map(run_view).collect()))
}

async fn run_detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<RunDetailResponse>> {
    let run_id = jcs_core::RunId::parse(id)?;
    let run = state
        .store
        .get_run(&run_id)
        .await?
        .ok_or_else(|| ApiError::not_found("run_not_found", format!("no run {run_id}")))?;

    let gates = state.store.list_gate_results(&run_id).await?;
    let task = state.store.get_task(&run.task_id).await?;

    Ok(Json(RunDetailResponse {
        run: run_view(&run),
        gates: gates
            .iter()
            .map(|gate| GateView {
                name: gate.name.clone(),
                status: gate.status.as_str().to_string(),
                exit_code: gate.exit_code,
                duration_ms: gate.duration_ms,
            })
            .collect(),
        task: task.as_ref().map(task_view),
    }))
}

async fn ask(
    State(state): State<AppState>,
    Json(request): Json<AskRequest>,
) -> ApiResult<Json<AskResponse>> {
    if is_conversational(&request.prompt) {
        return Err(ApiError::bad_request(
            "not_an_objective",
            "that looks like a conversation, not an objective; use the Orchestrator chat,              or describe a concrete change to execute",
        ));
    }
    let project = resolve_project(&state, &request.project).await?;

    // Fail before creating state if the repository is not usable.
    let repo = jcs_git::Repo::new(&project.root_path);
    repo.require_repository().await.map_err(|error| {
        ApiError::bad_request(
            "project_unusable",
            format!("project {} is not usable: {error}", project.name),
        )
    })?;

    let permissions = match request.permissions {
        jcs_proto::PermissionMode::Strict => "strict",
        jcs_proto::PermissionMode::Auto => "auto",
    };

    let goal = state
        .store
        .create_goal_with_settings(
            &project.id,
            &request.prompt,
            request.model.as_deref(),
            request.agent.as_deref(),
            permissions,
        )
        .await?;

    if request.plan {
        spawn_planning(state.clone(), goal.clone(), project.clone());
        return Ok(Json(AskResponse {
            goal_id: goal.id.to_string(),
            planned: true,
            task_id: None,
        }));
    }

    // Single-task mode: the scheduler still dispatches it, so the lease and budget
    // rules apply to it exactly as they do to planned tasks.
    let title = request
        .title
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| first_line(&request.prompt, 80));
    let task = state.store.create_task(&goal.id, &title).await?;

    Ok(Json(AskResponse {
        goal_id: goal.id.to_string(),
        planned: false,
        task_id: Some(task.id.to_string()),
    }))
}

/// Runs the planner in the background so the client is not held for a model call.
pub(crate) fn spawn_planning(state: AppState, goal: Goal, project: Project) {
    tokio::spawn(async move {
        match crate::planner::plan_goal(&state, &goal, &project).await {
            Ok(plan) => {
                let raw = serde_json::to_string(&plan).unwrap_or_default();
                if let Err(error) = state
                    .store
                    .create_planned_tasks(&goal.id, &plan, &raw)
                    .await
                {
                    fail_planning(&state, &goal, &format!("{error:#}")).await;
                } else {
                    tracing::info!(
                        goal = %goal.id,
                        tasks = plan.tasks.len(),
                        "goal planned"
                    );
                }
            }
            Err(error) => fail_planning(&state, &goal, &format!("{error:#}")).await,
        }
    });
}

async fn fail_planning(state: &AppState, goal: &Goal, reason: &str) {
    tracing::error!(goal = %goal.id, reason, "planning failed");

    // `pending -> failed` is not a legal transition: a goal that never started cannot have
    // failed. Blocking keeps the objective and its text so the operator can retry it.
    if let Err(error) = state
        .store
        .set_goal_status(&goal.id, GoalStatus::Blocked)
        .await
    {
        tracing::warn!(error = %error, "cannot mark the goal as blocked");
    }

    let event = NewEvent::new(EventKind::Error)
        .project(&goal.project_id)
        .goal(&goal.id)
        .payload(json!({
            "category": "planning_failed",
            "reason": reason,
            "hint": "the objective was not decomposed; fix the cause and `jcs resume` it, or run it with `jcs ask --no-plan` as a single task",
        }));
    if let Err(error) = jcs_store::append_event(&state.store, event).await {
        tracing::warn!(error = %error, "cannot record the planning failure");
    }
}

/// Iterates a task: retries it with feedback when it failed/was blocked, or creates a
/// dependent follow-up task when it already succeeded. A task is never silently reopened.
async fn iterate_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<TaskIterateRequest>,
) -> ApiResult<Json<TaskActionResponse>> {
    let task_id = TaskId::parse(id)?;
    let task = state
        .store
        .get_task(&task_id)
        .await?
        .ok_or_else(|| ApiError::not_found("task_not_found", format!("no task {task_id}")))?;
    let goal = state.store.get_goal(&task.goal_id).await?.ok_or_else(|| {
        ApiError::not_found("goal_not_found", format!("no goal {}", task.goal_id))
    })?;

    if matches!(
        goal.status,
        GoalStatus::Succeeded | GoalStatus::Failed | GoalStatus::Cancelled
    ) {
        return Err(ApiError::conflict(
            "goal_finished",
            "the goal already finished; create a new objective to iterate on it",
        ));
    }

    let feedback = request.feedback.trim();
    match task.status {
        TaskStatus::Running | TaskStatus::Verifying => Err(ApiError::conflict(
            "task_busy",
            "the task is still running; let it finish or abort it before iterating",
        )),
        TaskStatus::Cancelled => Err(ApiError::conflict(
            "task_cancelled",
            "a cancelled task cannot be reopened; create a new objective",
        )),
        TaskStatus::Succeeded => {
            if feedback.is_empty() {
                return Err(ApiError::bad_request(
                    "empty_feedback",
                    "describe what should change before iterating a finished task",
                ));
            }
            let created = state.store.add_iteration_task(&task, feedback).await?;
            if goal.status == GoalStatus::Blocked {
                state
                    .store
                    .set_goal_status(&goal.id, GoalStatus::Running)
                    .await?;
            }
            Ok(Json(TaskActionResponse {
                ok: true,
                task_id: created.id.to_string(),
                status: TaskStatus::Pending.as_str().to_string(),
                created_task_id: Some(created.id.to_string()),
            }))
        }
        TaskStatus::Pending => {
            if !feedback.is_empty() {
                state
                    .store
                    .set_task_instructions(&task.id, feedback)
                    .await?;
            }
            if goal.status == GoalStatus::Blocked {
                state
                    .store
                    .set_goal_status(&goal.id, GoalStatus::Running)
                    .await?;
            }
            Ok(Json(TaskActionResponse {
                ok: true,
                task_id: task.id.to_string(),
                status: TaskStatus::Pending.as_str().to_string(),
                created_task_id: None,
            }))
        }
        TaskStatus::Failed | TaskStatus::Blocked => {
            let retried = state.store.retry_task(&task.id, Some(feedback)).await?;
            if goal.status == GoalStatus::Blocked {
                state
                    .store
                    .set_goal_status(&goal.id, GoalStatus::Running)
                    .await?;
            }
            Ok(Json(TaskActionResponse {
                ok: true,
                task_id: retried.id.to_string(),
                status: retried.status.as_str().to_string(),
                created_task_id: None,
            }))
        }
    }
}

/// Clears a failed integration so the merge queue tries again. The operator is expected to
/// have resolved the conflict in the task worktree.
async fn retry_task_merge(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<TaskActionResponse>> {
    let task_id = TaskId::parse(id)?;
    let task = state
        .store
        .get_task(&task_id)
        .await?
        .ok_or_else(|| ApiError::not_found("task_not_found", format!("no task {task_id}")))?;
    if task.status != TaskStatus::Succeeded {
        return Err(ApiError::conflict(
            "task_not_succeeded",
            "only a succeeded task can retry its integration",
        ));
    }
    if state.store.task_merge_error(&task_id).await?.is_none() {
        return Err(ApiError::conflict(
            "no_merge_error",
            "the task has no integration error to retry",
        ));
    }

    state.store.clear_task_merge_error(&task_id).await?;
    if let Some(goal) = state.store.get_goal(&task.goal_id).await? {
        if goal.status == GoalStatus::Blocked {
            state
                .store
                .set_goal_status(&goal.id, GoalStatus::Running)
                .await?;
        }
    }

    Ok(Json(TaskActionResponse {
        ok: true,
        task_id: task.id.to_string(),
        status: TaskStatus::Succeeded.as_str().to_string(),
        created_task_id: None,
    }))
}

/// Bounded event history for one task, used by the interface to reconstruct the agent's
/// trace when it connects after the task started.
async fn task_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<TaskEventsQuery>,
) -> ApiResult<Json<Vec<jcs_core::Event>>> {
    let id = TaskId::parse(id)?;
    let events = jcs_store::events_for_task(&state.store, &id, query.limit.unwrap_or(500)).await?;
    Ok(Json(events))
}

#[derive(Debug, Deserialize)]
struct TaskEventsQuery {
    #[serde(default)]
    limit: Option<i64>,
}

async fn abort(
    State(state): State<AppState>,
    Json(request): Json<jcs_proto::AbortRequest>,
) -> ApiResult<Json<jcs_proto::OkResponse>> {
    let run_id = jcs_core::RunId::parse(request.run_id)?;
    abort_run(&state, &run_id).await?;
    Ok(Json(jcs_proto::OkResponse { ok: true }))
}

/// Stops one run, its task and, if needed, its goal. Shared by the abort button and by
/// cancelling a whole objective.
async fn abort_run(state: &AppState, run_id: &jcs_core::RunId) -> ApiResult<()> {
    let run = state
        .store
        .get_run(run_id)
        .await?
        .ok_or_else(|| ApiError::not_found("run_not_found", format!("no run {run_id}")))?;

    if run.status != RunStatus::Started {
        return Err(ApiError::conflict(
            "run_finished",
            format!("run {run_id} already ended as {}", run.status.as_str()),
        ));
    }

    if let Some(task) = state.store.get_task(&run.task_id).await? {
        if let Some(session_id) = task.session_id.as_deref() {
            if let Ok(agent) = state.agent.get().await {
                if let Err(error) = agent.interrupt(session_id).await {
                    tracing::warn!(error = %error, "cannot interrupt the agent session");
                }
            }
        }

        if matches!(task.status, TaskStatus::Running | TaskStatus::Verifying) {
            if let Err(error) = state
                .store
                .set_task_status(&task.id, TaskStatus::Cancelled)
                .await
            {
                tracing::warn!(error = %error, "cannot cancel the task");
            }
        }

        if let Some(goal) = state.store.get_goal(&task.goal_id).await? {
            if goal.status.can_transition_to(GoalStatus::Cancelled) {
                if let Err(error) = state
                    .store
                    .set_goal_status(&goal.id, GoalStatus::Cancelled)
                    .await
                {
                    tracing::warn!(error = %error, "cannot cancel the goal");
                }
            }
        }
    }

    state
        .store
        .finish_run(
            run_id,
            RunStatus::Cancelled,
            None,
            Some("aborted by client".to_string()),
        )
        .await?;

    Ok(())
}

/// Stops a whole objective: every started run, every non-terminal task, then the goal.
async fn cancel_goal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<jcs_proto::GoalActionResponse>> {
    let goal_id = jcs_core::GoalId::parse(id)?;
    let goal = state
        .store
        .get_goal(&goal_id)
        .await?
        .ok_or_else(|| ApiError::not_found("goal_not_found", format!("no goal {goal_id}")))?;

    if matches!(goal.status, GoalStatus::Succeeded | GoalStatus::Failed) {
        return Err(ApiError::conflict(
            "goal_finished",
            format!("goal {goal_id} is already {}", goal.status.as_str()),
        ));
    }

    for task in state.store.list_tasks_by_goal(&goal_id).await? {
        for run in state.store.list_runs_by_task(&task.id).await? {
            if run.status == RunStatus::Started {
                abort_run(&state, &run.id).await?;
            }
        }
        if let Some(current) = state.store.get_task(&task.id).await? {
            if !current.status.is_terminal() {
                if let Err(error) = state
                    .store
                    .set_task_status(&task.id, TaskStatus::Cancelled)
                    .await
                {
                    tracing::warn!(error = %error, "cannot cancel the task");
                }
            }
        }
    }

    // An aborted run may already have cancelled the goal; re-read before transitioning.
    let current = state
        .store
        .get_goal(&goal_id)
        .await?
        .ok_or_else(|| ApiError::not_found("goal_not_found", format!("no goal {goal_id}")))?;
    if current.status.can_transition_to(GoalStatus::Cancelled) {
        state
            .store
            .set_goal_status(&goal_id, GoalStatus::Cancelled)
            .await?;
    }

    Ok(Json(jcs_proto::GoalActionResponse {
        ok: true,
        goal_id: goal_id.to_string(),
        status: GoalStatus::Cancelled.as_str().to_string(),
    }))
}

/// Deletes a goal and its tasks, runs and leases. Running work must be cancelled first:
/// deleting a goal whose agent is still executing would leave an orphan session.
async fn delete_goal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<jcs_proto::OkResponse>> {
    let goal_id = jcs_core::GoalId::parse(id)?;
    let goal = state
        .store
        .get_goal(&goal_id)
        .await?
        .ok_or_else(|| ApiError::not_found("goal_not_found", format!("no goal {goal_id}")))?;

    let tasks = state.store.list_tasks_by_goal(&goal_id).await?;
    if tasks
        .iter()
        .any(|task| matches!(task.status, TaskStatus::Running | TaskStatus::Verifying))
    {
        return Err(ApiError::conflict(
            "goal_busy",
            "the goal still has running work; cancel it and wait for the tasks to settle",
        ));
    }

    let previous = goal.status;
    if previous.can_transition_to(GoalStatus::Cancelled) {
        // Stop the scheduler from dispatching a pending task between this check and the
        // delete. The row is removed immediately afterwards.
        state
            .store
            .set_goal_status(&goal_id, GoalStatus::Cancelled)
            .await?;
    }

    state.store.delete_goal(&goal_id).await?;

    let mut event = NewEvent::new(EventKind::GoalDeleted)
        .project(&goal.project_id)
        .goal(&goal_id);
    event.payload = json!({
        "prompt": goal.prompt,
        "previous_status": previous.as_str(),
    });
    if let Err(error) = jcs_store::append_event(state.store.as_ref(), event).await {
        tracing::warn!(error = %error, "cannot record the goal deletion");
    }

    Ok(Json(jcs_proto::OkResponse { ok: true }))
}

#[derive(Debug, Deserialize)]
struct ProjectQuery {
    /// Project name or absolute path.
    project: String,
}

/// Exact greetings only. A message like "hola, arregla el login" stays a valid objective.
fn is_conversational(prompt: &str) -> bool {
    let normalized: String = prompt
        .trim()
        .to_lowercase()
        .chars()
        .filter(|character| character.is_alphanumeric() || character.is_whitespace())
        .collect();
    let normalized = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    matches!(
        normalized.as_str(),
        "hola"
            | "buenas"
            | "buenos dias"
            | "buenas tardes"
            | "buenas noches"
            | "hey"
            | "hi"
            | "hello"
            | "que tal"
            | "como estas"
            | "gracias"
            | "ok"
            | "vale"
    )
}

async fn resolve_project(state: &AppState, reference: &str) -> ApiResult<Project> {
    state
        .store
        .resolve_project(reference)
        .await?
        .ok_or_else(|| {
            ApiError::not_found(
                "project_not_found",
                format!("no project matches {reference:?}; add it with `jcs project add <path>`"),
            )
        })
}

fn compile_policy(state: &AppState, project: &Project) -> ApiResult<jcs_policy::Compiled> {
    jcs_policy::compile(
        &project.root_path,
        &state.packs,
        &state.config.default_packs,
    )
    .map_err(|error| ApiError::bad_request("policy_error", error.to_string()))
}

fn parser_label(parser: jcs_gates::ParserKind) -> &'static str {
    match parser {
        jcs_gates::ParserKind::ExitCode => "exit_code",
        jcs_gates::ParserKind::Generic => "generic",
        jcs_gates::ParserKind::Cargo => "cargo",
        jcs_gates::ParserKind::Node => "node",
        jcs_gates::ParserKind::Coverage => "coverage",
    }
}

fn file_state_label(state: jcs_policy::FileState) -> &'static str {
    match state {
        jcs_policy::FileState::Created => "created",
        jcs_policy::FileState::Updated => "updated",
        jcs_policy::FileState::Unchanged => "unchanged",
        jcs_policy::FileState::Drifted => "drifted",
        jcs_policy::FileState::Missing => "missing",
        jcs_policy::FileState::Refused => "refused",
    }
}

fn gate_views(compiled: &jcs_policy::Compiled) -> Vec<PolicyGateView> {
    compiled
        .composition
        .gates
        .iter()
        .map(|(name, entry)| PolicyGateView {
            name: name.clone(),
            command: entry.def.command.clone(),
            timeout_secs: entry.def.timeout.as_secs(),
            required: entry.def.required,
            parser: parser_label(entry.def.parser).to_string(),
            source: entry.source.clone(),
        })
        .collect()
}

async fn policy(
    State(state): State<AppState>,
    Query(query): Query<ProjectQuery>,
) -> ApiResult<Json<PolicyView>> {
    let project = resolve_project(&state, &query.project).await?;
    let compiled = compile_policy(&state, &project)?;

    // Drift is read-only here: report what `render --check` would say, never write.
    let report = jcs_policy::apply(&compiled, &project.root_path, true, false)
        .map_err(|error| ApiError::bad_request("policy_error", error.to_string()))?;
    let drift = report
        .files
        .iter()
        .filter(|file| file.state != jcs_policy::FileState::Unchanged)
        .map(|file| file.path.clone())
        .collect();

    Ok(Json(PolicyView {
        project: project.name.clone(),
        packs: compiled
            .composition
            .packs
            .iter()
            .map(|pack| PolicyPackView {
                id: pack.id.clone(),
                version: pack.version.clone(),
                hash: pack.hash.clone(),
                description: pack.description.clone(),
            })
            .collect(),
        rules: compiled
            .composition
            .rules
            .iter()
            .map(|(key, entry)| PolicyRuleView {
                key: key.clone(),
                value: entry.value.clone(),
                source: entry.source.clone(),
            })
            .collect(),
        gates: gate_views(&compiled),
        skills: compiled.composition.skills.clone(),
        drift,
    }))
}

async fn policy_packs(State(state): State<AppState>) -> Json<Vec<PackCatalogEntry>> {
    Json(
        state
            .packs
            .values()
            .map(|pack| PackCatalogEntry {
                id: pack.id.clone(),
                version: pack.version.clone(),
                description: pack.description.clone(),
                gates: pack.gates.gates.len(),
                requires: pack.requires.clone(),
            })
            .collect(),
    )
}

async fn render_policy(
    State(state): State<AppState>,
    Json(request): Json<RenderPolicyRequest>,
) -> ApiResult<Json<RenderPolicyResponse>> {
    let project = resolve_project(&state, &request.project).await?;
    let compiled = compile_policy(&state, &project)?;

    let report = jcs_policy::apply(&compiled, &project.root_path, request.check, request.force)
        .map_err(|error| ApiError::bad_request("policy_error", error.to_string()))?;

    let files: Vec<RenderedFileView> = report
        .files
        .iter()
        .map(|file| RenderedFileView {
            path: file.path.clone(),
            state: file_state_label(file.state).to_string(),
            detail: file.detail.clone(),
        })
        .collect();

    if !request.check {
        let event = NewEvent::new(EventKind::PolicyRendered)
            .project(&project.id)
            .payload(json!({
                "packs": compiled.composition.pack_ids(),
                "files": files
                    .iter()
                    .map(|file| json!({ "path": file.path, "state": file.state }))
                    .collect::<Vec<_>>(),
            }));
        jcs_store::append_event(&state.store, event).await?;
    }

    Ok(Json(RenderPolicyResponse {
        ok: report.ok(),
        check: request.check,
        files,
    }))
}

async fn gates(
    State(state): State<AppState>,
    Query(query): Query<ProjectQuery>,
) -> ApiResult<Json<Vec<PolicyGateView>>> {
    let project = resolve_project(&state, &query.project).await?;
    let compiled = compile_policy(&state, &project)?;
    Ok(Json(gate_views(&compiled)))
}

async fn run_gates(
    State(state): State<AppState>,
    Json(request): Json<GateRunRequest>,
) -> ApiResult<Json<GateRunResponse>> {
    let project = resolve_project(&state, &request.project).await?;
    let compiled = compile_policy(&state, &project)?;
    let mut set = compiled.composition.gate_set();

    if let Some(name) = &request.gate {
        let def = set.gates.get(name).cloned().ok_or_else(|| {
            ApiError::not_found(
                "gate_not_found",
                format!("no gate {name:?} in the project policy; run `jcs gates list`"),
            )
        })?;
        set.gates.clear();
        set.gates.insert(name.clone(), def);
    }

    if set.is_empty() {
        return Err(ApiError::bad_request(
            "no_gates",
            "this project has no gates; select packs in .jcs/policy.toml and run `jcs policy render`",
        ));
    }

    // Runs in the main checkout on purpose: this is a diagnostic command, not part of a
    // task. Task execution always runs gates inside the task's isolated worktree.
    let outcomes = jcs_gates::run_all(&set, &project.root_path).await;
    let ok = jcs_gates::GateSet::all_required_passed(&outcomes, &set);

    Ok(Json(GateRunResponse {
        ok,
        outcomes: outcomes
            .iter()
            .map(|outcome| GateOutcomeView {
                name: outcome.name.clone(),
                status: outcome.status.as_str().to_string(),
                exit_code: outcome.exit_code,
                duration_ms: outcome.duration_ms,
                excerpt: outcome.excerpt.clone(),
                findings: outcome
                    .findings
                    .iter()
                    .map(|finding| finding.message.clone())
                    .collect(),
            })
            .collect(),
    }))
}

async fn goal_detail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<jcs_proto::GoalDetailResponse>> {
    let goal_id = jcs_core::GoalId::parse(id)?;
    let goal = state
        .store
        .get_goal(&goal_id)
        .await?
        .ok_or_else(|| ApiError::not_found("goal_not_found", format!("no goal {goal_id}")))?;

    let tasks = state.store.list_tasks_by_goal(&goal_id).await?;
    let mut runs = Vec::new();
    let views = task_views(&state, &tasks).await?;
    for task in &tasks {
        runs.extend(state.store.list_runs_by_task(&task.id).await?);
    }
    runs.sort_by_key(|run| std::cmp::Reverse(run.started_at));

    Ok(Json(jcs_proto::GoalDetailResponse {
        goal: goal_view(&goal),
        tasks: views,
        runs: runs.iter().map(run_view).collect(),
    }))
}

async fn resume_goal(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<jcs_proto::GoalActionResponse>> {
    let goal_id = jcs_core::GoalId::parse(id)?;
    let goal = state
        .store
        .get_goal(&goal_id)
        .await?
        .ok_or_else(|| ApiError::not_found("goal_not_found", format!("no goal {goal_id}")))?;

    if goal.status != GoalStatus::Blocked {
        return Err(ApiError::conflict(
            "goal_not_blocked",
            format!("goal {goal_id} is {}, not blocked", goal.status.as_str()),
        ));
    }

    state.store.clear_goal_merge_errors(&goal_id).await?;
    state
        .store
        .set_goal_status(&goal_id, GoalStatus::Running)
        .await?;

    Ok(Json(jcs_proto::GoalActionResponse {
        ok: true,
        goal_id: goal_id.to_string(),
        status: GoalStatus::Running.as_str().to_string(),
    }))
}

/// Models available to the agent backend. The interface uses this instead of asking the
/// operator to remember `provider/model` strings and prices.
async fn catalog_models(State(state): State<AppState>) -> ApiResult<Json<Vec<ModelCatalogEntry>>> {
    let agent = state.agent.get().await?;
    let models = agent.model_catalog().await?;
    Ok(Json(
        models
            .into_iter()
            .map(|model| ModelCatalogEntry {
                reference: model.reference,
                name: model.name,
                cost_input: model.cost_input,
                cost_output: model.cost_output,
            })
            .collect(),
    ))
}

/// Conversation with the project-level orchestrator, oldest first.
async fn orchestrator_history(
    State(state): State<AppState>,
    Query(query): Query<ProjectQuery>,
) -> ApiResult<Json<Vec<OrchestratorMessage>>> {
    let project = resolve_project(&state, &query.project).await?;
    let messages = state.store.orchestrator_messages(&project.id, 200).await?;
    Ok(Json(
        messages
            .into_iter()
            .map(|message| OrchestratorMessage {
                role: message.role,
                text: message.text,
                created_at: message.created_at,
            })
            .collect(),
    ))
}

/// One conversational turn with the orchestrator. The HTTP call waits for the answer; the
/// desktop app runs it off the UI thread and shows a spinner.
async fn orchestrator_chat(
    State(state): State<AppState>,
    Json(request): Json<OrchestratorChatRequest>,
) -> ApiResult<Json<jcs_proto::OkResponse>> {
    let prompt = request.prompt.trim();
    if prompt.is_empty() {
        return Err(ApiError::bad_request(
            "empty_message",
            "write a message for the orchestrator",
        ));
    }
    let project = resolve_project(&state, &request.project).await?;

    // One turn at a time. The guard is moved into the background task and released when
    // the answer finishes; the app receives the text through the normal event stream.
    let guard = state
        .orchestrator_guard
        .clone()
        .try_lock_owned()
        .map_err(|_| {
            ApiError::conflict(
                "orchestrator_busy",
                "the orchestrator is still answering the previous message",
            )
        })?;

    state
        .store
        .append_orchestrator_message(&project.id, "user", prompt)
        .await?;
    crate::orchestrator::spawn_turn(state.clone(), project, prompt.to_string(), guard);

    Ok(Json(jcs_proto::OkResponse { ok: true }))
}

async fn orchestrator_reset(
    State(state): State<AppState>,
    Query(query): Query<ProjectQuery>,
) -> ApiResult<Json<jcs_proto::OkResponse>> {
    let project = resolve_project(&state, &query.project).await?;
    let _guard = state
        .orchestrator_guard
        .clone()
        .try_lock_owned()
        .map_err(|_| {
            ApiError::conflict(
                "orchestrator_busy",
                "wait for the current answer before clearing the conversation",
            )
        })?;
    if let Some(session_id) = state.store.orchestrator_session(&project.id).await? {
        if let Ok(agent) = state.agent.get().await {
            if let Err(error) = agent.interrupt(&session_id).await {
                tracing::debug!(error = %error, "cannot interrupt the orchestrator session");
            }
        }
    }
    state.store.clear_orchestrator(&project.id).await?;
    Ok(Json(jcs_proto::OkResponse { ok: true }))
}

/// Skills installed for the project, as the backend lists them.
async fn catalog_skills(
    State(state): State<AppState>,
    Query(query): Query<ProjectQuery>,
) -> ApiResult<Json<Vec<SkillCatalogEntry>>> {
    let project = resolve_project(&state, &query.project).await?;
    let agent = state.agent.get().await?;
    let skills = agent.skills(&project.root_path).await?;
    Ok(Json(
        skills
            .into_iter()
            .map(|skill| SkillCatalogEntry {
                id: skill.id,
                name: skill.name,
                description: skill.description,
                autoinvoke: skill.autoinvoke,
                path: skill.path,
            })
            .collect(),
    ))
}

async fn catalog_mcp(
    State(state): State<AppState>,
    Query(query): Query<ProjectQuery>,
) -> ApiResult<Json<Vec<McpServerEntry>>> {
    let project = resolve_project(&state, &query.project).await?;
    let agent = state.agent.get().await?;
    let servers = agent.mcp_servers(&project.root_path).await?;
    Ok(Json(
        servers
            .into_iter()
            .map(|server| McpServerEntry {
                name: server.name,
                status: server.status,
                title: server.title,
                error: server.error,
            })
            .collect(),
    ))
}

async fn connect_mcp_server(
    State(state): State<AppState>,
    Path(server): Path<String>,
    Query(query): Query<ProjectQuery>,
) -> ApiResult<Json<jcs_proto::OkResponse>> {
    let project = resolve_project(&state, &query.project).await?;
    let agent = state.agent.get().await?;
    agent.connect_mcp(&server, &project.root_path).await?;
    Ok(Json(jcs_proto::OkResponse { ok: true }))
}

async fn disconnect_mcp_server(
    State(state): State<AppState>,
    Path(server): Path<String>,
    Query(query): Query<ProjectQuery>,
) -> ApiResult<Json<jcs_proto::OkResponse>> {
    let project = resolve_project(&state, &query.project).await?;
    let agent = state.agent.get().await?;
    agent.disconnect_mcp(&server, &project.root_path).await?;
    Ok(Json(jcs_proto::OkResponse { ok: true }))
}

async fn add_mcp_server(
    State(state): State<AppState>,
    Json(request): Json<McpAddRequest>,
) -> ApiResult<Json<jcs_proto::OkResponse>> {
    let project = resolve_project(&state, &request.project).await?;
    let agent = state.agent.get().await?;
    agent
        .add_mcp(&request.server, request.config, &project.root_path)
        .await?;
    Ok(Json(jcs_proto::OkResponse { ok: true }))
}

async fn remove_mcp_server(
    State(state): State<AppState>,
    Path(server): Path<String>,
    Query(query): Query<ProjectQuery>,
) -> ApiResult<Json<jcs_proto::OkResponse>> {
    let project = resolve_project(&state, &query.project).await?;
    let agent = state.agent.get().await?;
    agent.remove_mcp(&server, &project.root_path).await?;
    Ok(Json(jcs_proto::OkResponse { ok: true }))
}

async fn settings(State(state): State<AppState>) -> Json<SettingsView> {
    Json(settings_view(&state))
}

/// The view never contains a token: only whether one exists and its last four characters.
fn settings_view(state: &AppState) -> SettingsView {
    let mut view = state.runtime.view();
    view.telegram_configured = state.secrets.telegram_token().is_some();
    view.telegram_token_hint = state.secrets.telegram_hint();
    view
}

/// Applies a partial update, persists it and records who changed what.
///
/// Validation lives here, not in the interface: a second client must not be able to write
/// a timeout of zero or a negative concurrency just because it skipped the form.
async fn update_settings(
    State(state): State<AppState>,
    Json(patch): Json<SettingsPatch>,
) -> ApiResult<Json<SettingsView>> {
    let (values, mut changed) = state
        .runtime
        .update(&patch)
        .map_err(|message| ApiError::bad_request("invalid_settings", message))?;

    if let Some(token) = &patch.telegram_token {
        state
            .secrets
            .set_telegram_token((!token.trim().is_empty()).then(|| token.clone()))
            .map_err(|message| ApiError::bad_request("invalid_secret", message))?;
        changed.push("telegram_token".to_string());
        state.runtime.require_restart("telegram");
    }

    if !changed.is_empty() {
        // One row, complete values: a partial row would be worse than no row. The event
        // records the keys, never a secret (there are none in this table).
        let encoded = serde_json::to_string(&values)
            .map_err(|error| ApiError::internal(format!("cannot encode settings: {error}")))?;
        state.store.set_setting("runtime", &encoded).await?;

        let mut event = NewEvent::new(EventKind::SettingsChanged);
        event.payload = json!({ "changed": changed });
        jcs_store::append_event(state.store.as_ref(), event).await?;
    }

    Ok(Json(settings_view(&state)))
}

/// Prometheus-style text metrics.
///
/// Deliberately plain: a counter that a scraper can read and a human can `curl` without a
/// client library. Counts are computed from the bounded lists, which is accurate for the
/// dashboards this serves and documented as such.
async fn metrics(State(state): State<AppState>) -> ApiResult<axum::response::Response> {
    use std::collections::BTreeMap;

    let goals = state.store.list_goals(5_000).await?;
    let tasks = state.store.list_tasks(5_000).await?;
    let runs = state.store.list_runs(5_000).await?;
    let projects = state.store.list_projects().await?;

    let mut goal_counts: BTreeMap<&str, i64> = BTreeMap::new();
    for goal in &goals {
        *goal_counts.entry(goal.status.as_str()).or_default() += 1;
    }
    let mut task_counts: BTreeMap<&str, i64> = BTreeMap::new();
    for task in &tasks {
        *task_counts.entry(task.status.as_str()).or_default() += 1;
    }

    let cost: f64 = runs.iter().map(|run| run.cost_usd).sum();
    let tokens_in: i64 = runs.iter().map(|run| run.tokens_in).sum();
    let tokens_out: i64 = runs.iter().map(|run| run.tokens_out).sum();

    let mut body = String::new();
    body.push_str("# HELP jcs_up Whether the daemon is serving.\n# TYPE jcs_up gauge\njcs_up 1\n");
    body.push_str(&format!(
        "# HELP jcs_uptime_seconds Daemon uptime.\n# TYPE jcs_uptime_seconds gauge\njcs_uptime_seconds {}\n",
        state.uptime_secs()
    ));
    body.push_str(&format!(
        "# HELP jcs_projects_total Registered projects.\n# TYPE jcs_projects_total gauge\njcs_projects_total {}\n",
        projects.len()
    ));

    body.push_str("# HELP jcs_goals_total Goals by status.\n# TYPE jcs_goals_total gauge\n");
    for (status, count) in &goal_counts {
        body.push_str(&format!("jcs_goals_total{{status=\"{status}\"}} {count}\n"));
    }
    body.push_str("# HELP jcs_tasks_total Tasks by status.\n# TYPE jcs_tasks_total gauge\n");
    for (status, count) in &task_counts {
        body.push_str(&format!("jcs_tasks_total{{status=\"{status}\"}} {count}\n"));
    }

    body.push_str(&format!(
        "# HELP jcs_cost_usd_total Cumulative model spend.\n# TYPE jcs_cost_usd_total counter\njcs_cost_usd_total {cost:.6}\n"
    ));
    body.push_str(&format!(
        "# HELP jcs_tokens_total Tokens by direction.\n# TYPE jcs_tokens_total counter\njcs_tokens_total{{direction=\"in\"}} {tokens_in}\njcs_tokens_total{{direction=\"out\"}} {tokens_out}\n"
    ));

    // Per-project spend, so a dashboard can attribute cost instead of only seeing a total.
    body.push_str(
        "# HELP jcs_project_cost_usd_total Cumulative spend per project.\n# TYPE jcs_project_cost_usd_total counter\n",
    );
    for project in &projects {
        let spend = state
            .store
            .total_cost_usd(Some(&project.id))
            .await
            .unwrap_or(0.0);
        body.push_str(&format!(
            "jcs_project_cost_usd_total{{project=\"{}\"}} {spend:.6}\n",
            project.name.replace('"', "")
        ));
    }

    Ok(([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], body).into_response())
}

#[derive(Debug, Deserialize)]
struct MemorySearchQuery {
    project: String,
    q: String,
    #[serde(default)]
    limit: Option<usize>,
}

async fn memory_index(
    State(state): State<AppState>,
    Json(request): Json<jcs_proto::MemoryIndexRequest>,
) -> ApiResult<Json<jcs_proto::MemoryIndexResponse>> {
    let project = resolve_project(&state, &request.project).await?;

    let report = crate::memory_service::index_project(&state, &project, request.force)
        .await
        .ok_or_else(|| {
            ApiError::conflict(
                "index_in_progress",
                format!("{} is already being indexed", project.name),
            )
        })?;

    let event = NewEvent::new(EventKind::MemoryIndexed)
        .project(&project.id)
        .payload(json!({
            "files_indexed": report.files_indexed,
            "files_skipped": report.files_skipped,
            "files_removed": report.files_removed,
            "symbols": report.symbols,
            "chunks": report.chunks,
            "embeddings": report.embeddings,
            "duration_ms": report.duration_ms,
        }));
    jcs_store::append_event(&state.store, event).await?;

    Ok(Json(jcs_proto::MemoryIndexResponse {
        report: jcs_proto::MemoryIndexReportView {
            files_seen: report.files_seen,
            files_indexed: report.files_indexed,
            files_skipped: report.files_skipped,
            files_removed: report.files_removed,
            symbols: report.symbols,
            chunks: report.chunks,
            embeddings: report.embeddings,
            duration_ms: report.duration_ms,
            errors: report.errors,
        },
    }))
}

async fn memory_search(
    State(state): State<AppState>,
    Query(query): Query<MemorySearchQuery>,
) -> ApiResult<Json<jcs_proto::MemorySearchResponse>> {
    let project = resolve_project(&state, &query.project).await?;

    let hits = jcs_memory::search(
        &state.store,
        state.embeddings.as_deref(),
        &project.id,
        &query.q,
        query.limit.unwrap_or(10),
    )
    .await
    .map_err(|error| ApiError::internal(error.to_string()))?;

    Ok(Json(jcs_proto::MemorySearchResponse {
        query: query.q,
        hits: hits
            .iter()
            .map(|hit| jcs_proto::MemoryHitView {
                path: hit.path.clone(),
                start_line: hit.start_line,
                end_line: hit.end_line,
                score: hit.score,
                source: hit.source.as_str().to_string(),
                citation: hit.citation(),
                snippet: hit.snippet.clone(),
            })
            .collect(),
    }))
}

async fn memory_stats(
    State(state): State<AppState>,
    Query(query): Query<ProjectQuery>,
) -> ApiResult<Json<jcs_proto::MemoryStatsView>> {
    let project = resolve_project(&state, &query.project).await?;

    let stats = jcs_memory::stats(&state.store, &project.id)
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;

    Ok(Json(jcs_proto::MemoryStatsView {
        project: project.name.clone(),
        files: stats.files,
        symbols: stats.symbols,
        chunks: stats.chunks,
        embedded_chunks: stats.embedded_chunks,
        edges: stats.edges,
        last_indexed_at: stats.last_indexed_at,
        semantic_enabled: state.embeddings.is_some(),
    }))
}

#[derive(Debug, Deserialize)]
struct EventsQuery {
    #[serde(default)]
    since: i64,
}

/// Streams events as newline-delimited JSON.
///
/// The client reconnects with `since` = the last `seq` it saw; the server never emits
/// `seq <= since`, so resume is exact without relying on headers.
async fn events(State(state): State<AppState>, Query(query): Query<EventsQuery>) -> Response {
    let cursor = query.since.max(0);

    let body_stream = stream::try_unfold((state, cursor), |(state, cursor)| async move {
        loop {
            let events = jcs_store::events_since(&state.store, cursor, 200).await?;
            if events.is_empty() {
                // Nothing new: wait briefly instead of busy-looping the database.
                tokio::time::sleep(Duration::from_millis(250)).await;
                continue;
            }

            let mut buffer = Vec::new();
            let mut next = cursor;
            for event in events {
                next = event.seq;
                serde_json::to_writer(&mut buffer, &event)?;
                buffer.push(b'\n');
            }

            return Ok::<_, jcs_store::StoreError>(Some((Bytes::from(buffer), (state, next))));
        }
    });

    (
        [(header::CONTENT_TYPE, "application/x-ndjson")],
        Body::from_stream(body_stream),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    /// Start after this sequence. Omit for the most recent events.
    since: Option<i64>,
    limit: Option<i64>,
}

/// Bounded history, for `jcs logs` without `--follow` and for cursor lookup.
async fn event_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> ApiResult<Json<Vec<jcs_core::Event>>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 5_000);
    let events = match query.since {
        Some(since) => jcs_store::events_since(&state.store, since.max(0), limit).await?,
        None => jcs_store::recent_events(&state.store, limit).await?,
    };
    Ok(Json(events))
}

fn first_line(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    let truncated = jcs_git::truncate_words(line, max);
    if truncated.trim().is_empty() {
        "untitled task".to_string()
    } else {
        truncated
    }
}

fn project_view(project: &Project) -> ProjectView {
    ProjectView {
        id: project.id.to_string(),
        name: project.name.clone(),
        root_path: project.root_path.to_string_lossy().to_string(),
        base_branch: project.base_branch.clone(),
        created_at: project.created_at,
        auto_merge: project.auto_merge,
        budget_usd_per_day: project.budget_usd_per_day,
        budget_tokens_per_day: project.budget_tokens_per_day,
    }
}

fn goal_view(goal: &jcs_core::Goal) -> GoalView {
    GoalView {
        id: goal.id.to_string(),
        project_id: goal.project_id.to_string(),
        prompt: goal.prompt.clone(),
        status: goal.status.as_str().to_string(),
        created_at: goal.created_at,
        updated_at: goal.updated_at,
    }
}

fn task_view(task: &jcs_core::Task) -> TaskView {
    TaskView {
        id: task.id.to_string(),
        goal_id: task.goal_id.to_string(),
        title: task.title.clone(),
        instructions: task.instructions.clone(),
        status: task.status.as_str().to_string(),
        worktree: task
            .worktree
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        branch: task.branch.clone(),
        session_id: task.session_id.clone(),
        depends_on: Vec::new(),
        path_claims: task.path_claims.clone(),
        merged_at: None,
        merge_error: None,
        created_at: task.created_at,
        updated_at: task.updated_at,
    }
}

/// Adds the relationships the interface needs (dependencies and integration state).
async fn task_views(state: &AppState, tasks: &[jcs_core::Task]) -> ApiResult<Vec<TaskView>> {
    let mut views = Vec::with_capacity(tasks.len());
    for task in tasks {
        let mut view = task_view(task);
        view.depends_on = state
            .store
            .task_dependencies(&task.id)
            .await?
            .iter()
            .map(|id| id.to_string())
            .collect();
        view.merged_at = state.store.task_merged_at(&task.id).await?;
        view.merge_error = state.store.task_merge_error(&task.id).await?;
        views.push(view);
    }
    Ok(views)
}

fn run_view(run: &jcs_core::Run) -> RunView {
    RunView {
        id: run.id.to_string(),
        task_id: run.task_id.to_string(),
        attempt: run.attempt,
        status: run.status.as_str().to_string(),
        exit_code: run.exit_code,
        started_at: run.started_at,
        ended_at: run.ended_at,
        cost_usd: run.cost_usd,
        tokens_in: run.tokens_in,
        tokens_out: run.tokens_out,
        error: run.error.clone(),
    }
}

/// Records a daemon-level event: startup, shutdown and other lifecycle facts.
pub async fn record_lifecycle(state: &AppState, kind: EventKind, payload: serde_json::Value) {
    let event = NewEvent::new(kind).payload(payload);
    if let Err(error) = jcs_store::append_event(&state.store, event).await {
        tracing::error!(error = %error, "cannot record a lifecycle event");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn test_state() -> AppState {
        let store = Arc::new(jcs_store::Store::in_memory().await.unwrap());
        let config = Arc::new(crate::config::Config {
            socket_path: std::path::PathBuf::from("/tmp/jcs-test.sock"),
            db_path: std::path::PathBuf::from(":memory:"),
            worktrees_dir: std::env::temp_dir().join("jcs-test-worktrees"),
            max_concurrent: 1,
            default_model: None,
            default_agent: None,
            allow_shell: false,
            run_timeout_secs: 60,
            packs_dir: None,
            default_packs: Vec::new(),
            max_repair_cycles: 0,
            planning_model: None,
            planning_agent: None,
            lease_ttl_secs: 90,
            heartbeat_secs: 20,
            context_tokens: 0,
            memory_sweep_secs: 0,
            review: false,
            review_model: None,
            review_max_findings: 10,
            risk_max_lines: 400,
            telegram_token: None,
            telegram_chat_ids: Vec::new(),
        });
        let runtime = Arc::new(crate::settings::RuntimeSettings::from_config(&config));
        AppState::new(
            store,
            config,
            Arc::new(jcs_policy::loader::builtin_packs().unwrap()),
            None,
            runtime,
            Arc::new(crate::secrets::Secrets::in_memory(None)),
        )
    }

    async fn send(
        app: Router,
        method: &str,
        uri: &str,
        body: Option<&str>,
    ) -> (u16, serde_json::Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.unwrap_or_default().to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let status = response.status().as_u16();
        let bytes = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    #[tokio::test]
    async fn health_reports_the_protocol_version() {
        let (status, body) = send(router(test_state().await), "GET", "/health", None).await;
        assert_eq!(status, 200);
        assert_eq!(body["protocol"], jcs_proto::PROTOCOL_VERSION);
        assert_eq!(body["ok"], true);
    }

    #[tokio::test]
    async fn a_relative_project_path_is_rejected_with_a_reason() {
        let (status, body) = send(
            router(test_state().await),
            "POST",
            "/projects",
            Some(r#"{"path":"relative/repo"}"#),
        )
        .await;

        assert_eq!(status, 400);
        assert_eq!(body["code"], "relative_path");
        assert!(body["message"].as_str().unwrap().contains("absolute"));
    }

    #[tokio::test]
    async fn a_directory_that_is_not_a_repository_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let body = serde_json::json!({ "path": directory.path().to_string_lossy() });

        let (status, response) = send(
            router(test_state().await),
            "POST",
            "/projects",
            Some(&body.to_string()),
        )
        .await;

        assert_eq!(status, 400, "{response}");
        assert_eq!(response["code"], "not_a_repository");
    }

    #[tokio::test]
    async fn asking_for_an_unknown_project_suggests_the_next_step() {
        let (status, body) = send(
            router(test_state().await),
            "POST",
            "/ask",
            Some(r#"{"project":"nope","prompt":"do something"}"#),
        )
        .await;

        assert_eq!(status, 404);
        assert_eq!(body["code"], "project_not_found");
        assert!(body["message"].as_str().unwrap().contains("jcs project"));
    }

    #[tokio::test]
    async fn a_registered_repository_appears_in_the_list() {
        let directory = tempfile::tempdir().unwrap();
        let repo = jcs_git::Repo::new(directory.path());
        repo.init("main").await.unwrap();
        std::fs::write(directory.path().join("README.md"), "hi\n").unwrap();
        jcs_git::Repo::new(directory.path())
            .commit_all("chore: initial commit")
            .await
            .unwrap();

        let app = router(test_state().await);
        let request = serde_json::json!({
            "path": directory.path().to_string_lossy(),
            "name": "demo",
        });
        let (status, body) =
            send(app.clone(), "POST", "/projects", Some(&request.to_string())).await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["project"]["name"], "demo");
        assert_eq!(body["project"]["base_branch"], "main");

        let (status, list) = send(app, "GET", "/projects", None).await;
        assert_eq!(status, 200);
        assert_eq!(list["projects"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn titles_come_from_the_first_line_and_are_bounded() {
        assert_eq!(first_line("do the thing\nand more", 80), "do the thing");
        assert_eq!(first_line("   ", 80), "untitled task");
        assert_eq!(first_line(&"x".repeat(200), 10).chars().count(), 10);
        // Cut on a word boundary, not mid-word.
        assert_eq!(
            first_line(
                "create a file named hello.md containing exactly this line",
                40
            ),
            "create a file named hello.md containing"
        );
    }

    #[tokio::test]
    async fn a_goal_can_be_cancelled_and_deleted() {
        let state = test_state().await;
        let directory = tempfile::tempdir().unwrap();
        let project = Project {
            id: ProjectId::new(),
            name: "delete".to_string(),
            root_path: directory.path().to_path_buf(),
            base_branch: "main".to_string(),
            created_at: jiff::Timestamp::now(),
            auto_merge: false,
            budget_usd_per_day: None,
            budget_tokens_per_day: None,
        };
        state.store.insert_project(&project).await.unwrap();
        let goal = state
            .store
            .create_goal_with_settings(&project.id, "do it", None, None, "auto")
            .await
            .unwrap();
        let task = state.store.create_task(&goal.id, "task").await.unwrap();
        state
            .store
            .set_task_status(&task.id, TaskStatus::Running)
            .await
            .unwrap();
        let run = state.store.start_run(&task.id).await.unwrap();

        let app = router(state.clone());
        let (status, body) = send(
            app.clone(),
            "POST",
            &format!("/goals/{}/cancel", goal.id),
            Some("{}"),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(
            state
                .store
                .get_goal(&goal.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            GoalStatus::Cancelled
        );
        assert_eq!(
            state
                .store
                .get_task(&task.id)
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskStatus::Cancelled
        );
        assert_eq!(
            state.store.get_run(&run.id).await.unwrap().unwrap().status,
            RunStatus::Cancelled
        );

        let (status, body) = send(app, "DELETE", &format!("/goals/{}", goal.id), None).await;
        assert_eq!(status, 200, "{body}");
        assert!(state.store.get_goal(&goal.id).await.unwrap().is_none());
        assert!(state.store.get_task(&task.id).await.unwrap().is_none());
        let events = jcs_store::recent_events(state.store.as_ref(), 50)
            .await
            .unwrap();
        assert!(events
            .iter()
            .any(|event| event.kind == EventKind::GoalDeleted));
    }

    #[tokio::test]
    async fn a_finished_task_can_be_iterated_and_a_merge_retried() {
        let state = test_state().await;
        let directory = tempfile::tempdir().unwrap();
        let project = Project {
            id: ProjectId::new(),
            name: "iterate".to_string(),
            root_path: directory.path().to_path_buf(),
            base_branch: "main".to_string(),
            created_at: jiff::Timestamp::now(),
            auto_merge: false,
            budget_usd_per_day: None,
            budget_tokens_per_day: None,
        };
        state.store.insert_project(&project).await.unwrap();
        let goal = state
            .store
            .create_goal_with_settings(&project.id, "objective", None, None, "auto")
            .await
            .unwrap();
        let task = state.store.create_task(&goal.id, "original").await.unwrap();
        for status in [
            TaskStatus::Running,
            TaskStatus::Verifying,
            TaskStatus::Succeeded,
        ] {
            state.store.set_task_status(&task.id, status).await.unwrap();
        }

        let app = router(state.clone());
        let (status, body) = send(
            app.clone(),
            "POST",
            &format!("/tasks/{}/iterate", task.id),
            Some(r#"{"feedback":"change the header"}"#),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        let created = body["created_task_id"].as_str().unwrap().to_string();
        assert_eq!(body["status"], "pending");
        let created = jcs_core::TaskId::parse(created).unwrap();
        let created_task = state.store.get_task(&created).await.unwrap().unwrap();
        assert_eq!(
            created_task.instructions.as_deref(),
            Some("change the header")
        );
        assert_eq!(
            state.store.task_dependencies(&created).await.unwrap(),
            vec![task.id.clone()]
        );

        state
            .store
            .mark_task_merge_failed(&task.id, "conflict in src/a.rs")
            .await
            .unwrap();
        let (status, body) = send(
            app,
            "POST",
            &format!("/tasks/{}/merge-retry", task.id),
            Some("{}"),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        assert!(state
            .store
            .task_merge_error(&task.id)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn a_greeting_is_not_an_objective() {
        let (status, body) = send(
            router(test_state().await),
            "POST",
            "/ask",
            Some(r#"{"project":"demo","prompt":"Hola"}"#),
        )
        .await;
        assert_eq!(status, 400, "{body}");
        assert_eq!(body["code"], "not_an_objective");
    }

    #[tokio::test]
    async fn the_telegram_token_is_write_only() {
        let state = test_state().await;
        let app = router(state);
        let (status, body) = send(
            app.clone(),
            "PUT",
            "/settings",
            Some(r#"{"telegram_token":"fixture-token-abcd","telegram_chat_ids":[42]}"#),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["telegram_configured"], true);
        assert_eq!(body["telegram_token_hint"], "abcd");
        assert!(!body.to_string().contains("fixture-token"), "{body}");
        assert!(body["restart_required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "telegram"));

        let (status, body) = send(app, "GET", "/settings", None).await;
        assert_eq!(status, 200);
        assert_eq!(body["telegram_chat_ids"][0], 42);
        assert!(!body.to_string().contains("fixture-token"), "{body}");
    }

    #[tokio::test]
    async fn a_planning_failure_blocks_the_goal_instead_of_leaving_it_pending() {
        let state = test_state().await;
        let directory = tempfile::tempdir().unwrap();
        let project = Project {
            id: ProjectId::new(),
            name: "planner".to_string(),
            root_path: directory.path().to_path_buf(),
            base_branch: "main".to_string(),
            created_at: jiff::Timestamp::now(),
            auto_merge: false,
            budget_usd_per_day: None,
            budget_tokens_per_day: None,
        };
        state.store.insert_project(&project).await.unwrap();
        let goal = state
            .store
            .create_goal_with_settings(&project.id, "do it", None, None, "auto")
            .await
            .unwrap();

        fail_planning(&state, &goal, "the model is unavailable").await;

        let stored = state.store.get_goal(&goal.id).await.unwrap().unwrap();
        assert_eq!(stored.status, GoalStatus::Blocked);
        let events = jcs_store::recent_events(state.store.as_ref(), 20)
            .await
            .unwrap();
        assert!(events.iter().any(|event| {
            event.kind == EventKind::Error
                && event
                    .payload
                    .get("category")
                    .and_then(|value| value.as_str())
                    == Some("planning_failed")
        }));
    }

    #[tokio::test]
    async fn settings_are_validated_applied_and_audited() {
        let state = test_state().await;
        let app = router(state.clone());

        let (status, body) = send(app.clone(), "GET", "/settings", None).await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["run_timeout_secs"], 60);
        assert_eq!(body["review"], false);

        // Out-of-range values never reach the store.
        let (status, body) = send(
            app.clone(),
            "PUT",
            "/settings",
            Some(r#"{"run_timeout_secs":1}"#),
        )
        .await;
        assert_eq!(status, 400, "{body}");
        assert_eq!(body["code"], "invalid_settings");

        let (status, body) = send(
            app.clone(),
            "PUT",
            "/settings",
            Some(r#"{"review":true,"default_model":"cheap/model","max_concurrent":4}"#),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        assert_eq!(body["review"], true);
        assert_eq!(body["restart_required"][0], "max_concurrent");

        let (_, body) = send(app, "GET", "/settings", None).await;
        assert_eq!(body["default_model"], "cheap/model");

        let stored = state.store.setting("runtime").await.unwrap().unwrap();
        assert!(stored.contains("cheap/model"));
        let events = jcs_store::recent_events(state.store.as_ref(), 50)
            .await
            .unwrap();
        assert!(events
            .iter()
            .any(|event| event.kind == EventKind::SettingsChanged));
    }

    #[tokio::test]
    async fn run_views_expose_cost_and_tokens() {
        let run = jcs_core::Run {
            id: jcs_core::RunId::new(),
            task_id: jcs_core::TaskId::new(),
            attempt: 1,
            status: RunStatus::Succeeded,
            exit_code: Some(0),
            started_at: jiff::Timestamp::now(),
            ended_at: Some(jiff::Timestamp::now()),
            cost_usd: 0.5,
            tokens_in: 10,
            tokens_out: 20,
            error: None,
        };
        let view = run_view(&run);
        assert_eq!(view.status, "succeeded");
        assert_eq!(view.tokens_out, 20);
    }
}
