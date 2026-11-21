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
    EventKind, Goal, GoalStatus, NewEvent, Project, ProjectId, RunStatus, StatusMachine, TaskStatus,
};
use jcs_proto::{
    AddProjectRequest, AskRequest, AskResponse, ErrorBody, GateOutcomeView, GateRunRequest,
    GateRunResponse, GateView, GoalView, HealthResponse, PackCatalogEntry, PolicyGateView,
    PolicyPackView, PolicyRuleView, PolicyView, ProjectListResponse, ProjectResponse, ProjectView,
    RenderPolicyRequest, RenderPolicyResponse, RenderedFileView, RunDetailResponse, RunView,
    StatusResponse, TaskView, Totals, PROTOCOL_HEADER, PROTOCOL_VERSION,
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
        .route("/memory/index", post(memory_index))
        .route("/memory/search", get(memory_search))
        .route("/memory/stats", get(memory_stats))
        .route("/goals/{id}", get(goal_detail))
        .route("/goals/{id}/resume", post(resume_goal))
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
        tasks: tasks.iter().map(task_view).collect(),
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

    if let Err(error) = state
        .store
        .set_goal_status(&goal.id, GoalStatus::Failed)
        .await
    {
        tracing::warn!(error = %error, "cannot mark the goal as failed");
    }

    let event = NewEvent::new(EventKind::Error)
        .project(&goal.project_id)
        .goal(&goal.id)
        .payload(json!({
            "category": "planning_failed",
            "reason": reason,
            "hint": "the objective was not decomposed; run it with `jcs ask --no-plan`                      to execute it as a single task",
        }));
    if let Err(error) = jcs_store::append_event(&state.store, event).await {
        tracing::warn!(error = %error, "cannot record the planning failure");
    }
}

async fn abort(
    State(state): State<AppState>,
    Json(request): Json<jcs_proto::AbortRequest>,
) -> ApiResult<Json<jcs_proto::OkResponse>> {
    let run_id = jcs_core::RunId::parse(request.run_id)?;
    let run = state
        .store
        .get_run(&run_id)
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
            if let Err(error) = state
                .store
                .set_goal_status(&goal.id, jcs_core::GoalStatus::Cancelled)
                .await
            {
                tracing::debug!(error = %error, "goal already terminal");
            }
        }
    }

    state
        .store
        .finish_run(
            &run_id,
            RunStatus::Cancelled,
            None,
            Some("aborted by client".to_string()),
        )
        .await?;

    Ok(Json(jcs_proto::OkResponse { ok: true }))
}

#[derive(Debug, Deserialize)]
struct ProjectQuery {
    /// Project name or absolute path.
    project: String,
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
    let mut views = Vec::with_capacity(tasks.len());
    for task in &tasks {
        let mut view = task_view(task);
        view.depends_on = state
            .store
            .task_dependencies(&task.id)
            .await?
            .iter()
            .map(|id| id.to_string())
            .collect();
        views.push(view);
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
        status: task.status.as_str().to_string(),
        worktree: task
            .worktree
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        branch: task.branch.clone(),
        session_id: task.session_id.clone(),
        depends_on: Vec::new(),
        created_at: task.created_at,
        updated_at: task.updated_at,
    }
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
        AppState::new(
            store,
            config,
            Arc::new(jcs_policy::loader::builtin_packs().unwrap()),
            None,
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
