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
use jcs_core::{EventKind, NewEvent, Project, ProjectId, RunStatus, StatusMachine, TaskStatus};
use jcs_proto::{
    AddProjectRequest, AskRequest, AskResponse, ErrorBody, GateView, GoalView, HealthResponse,
    ProjectListResponse, ProjectResponse, ProjectView, RunDetailResponse, RunView, StatusResponse,
    TaskView, Totals, PROTOCOL_HEADER, PROTOCOL_VERSION,
};
use serde::Deserialize;

use crate::error::{ApiError, ApiResult};
use crate::executor::{self, TaskRequest};
use crate::state::AppState;

const LIST_LIMIT: i64 = 50;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/projects", get(list_projects).post(add_project))
        .route("/projects/{id}", delete(remove_project))
        .route("/goals", get(list_goals))
        .route("/tasks", get(list_tasks))
        .route("/runs", get(list_runs))
        .route("/runs/{id}", get(run_detail))
        .route("/ask", post(ask))
        .route("/abort", post(abort))
        .route("/events", get(events))
        .route("/events/history", get(event_history))
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
    };

    state.store.insert_project(&project).await?;
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
    let project = state
        .store
        .resolve_project(&request.project)
        .await?
        .ok_or_else(|| {
            ApiError::not_found(
                "project_not_found",
                format!(
                    "no project matches {:?}; run `jcs project list` or `jcs project add <path>`",
                    request.project
                ),
            )
        })?;

    // Fail before creating state if the repository is not usable.
    let repo = jcs_git::Repo::new(&project.root_path);
    repo.require_repository().await.map_err(|error| {
        ApiError::bad_request(
            "project_unusable",
            format!("project {} is not usable: {error}", project.name),
        )
    })?;

    let title = request
        .title
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| first_line(&request.prompt, 80));

    let goal = state
        .store
        .create_goal(&project.id, &request.prompt)
        .await?;
    let task = state.store.create_task(&goal.id, &title).await?;
    let run = state.store.start_run(&task.id).await?;

    executor::spawn(
        state.clone(),
        TaskRequest {
            task_id: task.id.clone(),
            run_id: run.id.clone(),
            model: request.model,
            agent: request.agent,
            permissions: request.permissions,
        },
    );

    Ok(Json(AskResponse {
        goal_id: goal.id.to_string(),
        task_id: task.id.to_string(),
        run_id: run.id.to_string(),
        task_status: task.status.as_str().to_string(),
    }))
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
        });
        AppState::new(store, config)
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
