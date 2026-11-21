//! Interface state: everything the window renders.
//!
//! The daemon is the source of truth. This module only mirrors its snapshots and events;
//! nothing here is durable and nothing here decides the status of a task.

use std::collections::VecDeque;

use jcs_core::Event;
use jcs_proto::{
    GateRunResponse, GoalActionResponse, HealthResponse, McpServerEntry, MemoryIndexResponse,
    MemorySearchResponse, MemoryStatsView, ModelCatalogEntry, PolicyView, ProjectView,
    RenderPolicyResponse, RunDetailResponse, SettingsView, SkillCatalogEntry, StatusResponse,
    TaskView, Totals,
};

/// How the window sees the daemon right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Connection {
    Connecting,
    Online,
    /// The daemon is unreachable. The message is shown verbatim; it already says what to do.
    Offline(String),
}

/// A message for the operator. Errors stay until dismissed; successes fade on the next one.
#[derive(Debug, Clone)]
pub struct Notice {
    pub message: String,
    pub error: bool,
}

/// One line of the activity console.
#[derive(Debug, Clone)]
pub struct ActivityLine {
    pub seq: i64,
    /// Goal the event belongs to, when it has one. Lets the interface show one objective.
    pub goal_id: Option<String>,
    /// Task the event belongs to, for the per-task agent trace.
    pub task_id: Option<String>,
    pub time: String,
    pub kind: String,
    pub detail: String,
}

/// Stage-based progress of one task. This is the lifecycle, not a guess at model progress:
/// there is no honest percentage for "how much thinking is left".
#[derive(Debug, Clone, Copy)]
pub struct TaskProgress {
    pub fraction: f32,
    pub label: &'static str,
}

pub fn task_progress(task: &TaskView) -> TaskProgress {
    match task.status.as_str() {
        "pending" => TaskProgress {
            fraction: 0.0,
            label: "en cola",
        },
        "running" => TaskProgress {
            fraction: 0.45,
            label: "agente trabajando",
        },
        "verifying" => TaskProgress {
            fraction: 0.8,
            label: "verificando gates",
        },
        "succeeded" => TaskProgress {
            fraction: 1.0,
            label: "completada",
        },
        "failed" => TaskProgress {
            fraction: 1.0,
            label: "falló",
        },
        "blocked" => TaskProgress {
            fraction: 0.5,
            label: "bloqueada",
        },
        "cancelled" => TaskProgress {
            fraction: 1.0,
            label: "cancelada",
        },
        _ => TaskProgress {
            fraction: 0.0,
            label: "desconocido",
        },
    }
}

#[derive(Debug, Default)]
pub struct State {
    pub connection: Option<Connection>,
    pub health: Option<HealthResponse>,
    pub projects: Vec<ProjectView>,
    pub goals: Vec<jcs_proto::GoalView>,
    pub tasks: Vec<jcs_proto::TaskView>,
    pub runs: Vec<jcs_proto::RunView>,
    pub totals: Totals,
    pub events: VecDeque<ActivityLine>,

    pub selected_project: Option<String>,
    pub selected_goal: Option<String>,
    pub selected_task: Option<String>,
    /// Whether the initial goal selection already happened. Closing the detail afterwards
    /// must stay closed instead of being undone by the next snapshot.
    pub auto_selected: bool,

    pub policy: Option<PolicyView>,
    pub rendered: Option<RenderPolicyResponse>,
    pub gates: Option<GateRunResponse>,
    pub memory: Option<MemoryStatsView>,
    pub memory_index: Option<MemoryIndexResponse>,
    pub memory_search: Option<MemorySearchResponse>,
    pub run_detail: Option<RunDetailResponse>,
    pub settings: Option<SettingsView>,
    pub models: Vec<ModelCatalogEntry>,
    pub skills: Vec<SkillCatalogEntry>,
    pub mcp: Vec<McpServerEntry>,
    pub notice: Option<Notice>,
}

impl State {
    pub fn apply(&mut self, update: crate::daemon::Update) {
        use crate::daemon::Update;

        match update {
            Update::Online(health) => {
                self.connection = Some(Connection::Online);
                self.health = Some(health);
            }
            Update::Offline(message) => {
                self.connection = Some(Connection::Offline(message));
                self.health = None;
            }
            Update::Status(status) => self.apply_status(status),
            Update::History(events) => self.merge_history(events),
            Update::TaskHistory(events) => self.merge_history(events),
            Update::Event(event) => self.push_event(event),
            Update::Notice { message, error } => self.notice = Some(Notice { message, error }),
            Update::Policy(policy) => self.policy = Some(policy),
            Update::Rendered(rendered) => {
                self.notice = Some(Notice {
                    message: if rendered.ok {
                        if rendered.check {
                            "la política está al día".to_string()
                        } else {
                            "política renderizada".to_string()
                        }
                    } else {
                        "la política tiene diferencias; revisa los archivos".to_string()
                    },
                    error: !rendered.ok,
                });
                self.rendered = Some(rendered);
            }
            Update::Gates(gates) => self.gates = Some(gates),
            Update::MemoryIndex(report) => self.memory_index = Some(report),
            Update::MemoryStats(stats) => self.memory = Some(stats),
            Update::MemorySearch(search) => self.memory_search = Some(search),
            Update::RunDetail(detail) => self.run_detail = Some(*detail),
            Update::GoalAction(action) => self.apply_goal_action(action),
            Update::GoalDeleted(goal_id) => {
                self.goals.retain(|goal| goal.id != goal_id);
                let task_ids: Vec<String> = self
                    .tasks
                    .iter()
                    .filter(|task| task.goal_id == goal_id)
                    .map(|task| task.id.clone())
                    .collect();
                self.tasks.retain(|task| task.goal_id != goal_id);
                self.runs.retain(|run| !task_ids.contains(&run.task_id));
                if self.selected_goal.as_deref() == Some(goal_id.as_str()) {
                    self.selected_goal = None;
                    self.selected_task = None;
                    self.run_detail = None;
                }
            }
            Update::Settings(settings) => self.settings = Some(settings),
            Update::Models(models) => self.models = models,
            Update::Skills(skills) => self.skills = skills,
            Update::Mcp(servers) => self.mcp = servers,
            Update::Project(project) => {
                self.selected_project = Some(project.id.clone());
                if let Some(existing) = self.projects.iter_mut().find(|p| p.id == project.id) {
                    *existing = project;
                } else {
                    self.projects.push(project);
                }
                // The next snapshot is canonical, but the window must be usable now.
            }
        }
        self.reconcile_selection();
    }

    fn apply_status(&mut self, status: StatusResponse) {
        self.health = Some(status.health);
        self.projects = status.projects;
        self.goals = status.goals;
        self.tasks = status.tasks;
        self.runs = status.runs;
        self.totals = status.totals;
        self.select_initial_goal();
    }

    fn apply_goal_action(&mut self, action: GoalActionResponse) {
        if let Some(goal) = self.goals.iter_mut().find(|g| g.id == action.goal_id) {
            goal.status = action.status.clone();
        }
        self.notice = Some(Notice {
            message: if action.status == "running" {
                format!(
                    "objetivo {} reanudado",
                    crate::daemon::short(&action.goal_id)
                )
            } else {
                format!(
                    "objetivo {} en estado {}",
                    crate::daemon::short(&action.goal_id),
                    action.status
                )
            },
            error: false,
        });
    }

    fn push_event(&mut self, event: Event) {
        // The history seed and the live stream overlap; the sequence is the identity.
        if self.events.back().is_some_and(|line| line.seq >= event.seq)
            && self.events.iter().any(|line| line.seq == event.seq)
        {
            return;
        }

        let detail = summarize(&event);
        self.events.push_back(ActivityLine {
            seq: event.seq,
            goal_id: event.goal_id.as_ref().map(|id| id.to_string()),
            task_id: event.task_id.as_ref().map(|id| id.to_string()),
            time: event.ts.strftime("%H:%M:%S").to_string(),
            kind: event.kind.as_str().to_string(),
            detail,
        });
        while self.events.len() > 2000 {
            self.events.pop_front();
        }
    }

    /// Merges historical events (global or per task) with the live stream.
    pub fn merge_history(&mut self, events: Vec<Event>) {
        for event in events {
            self.push_event(event);
        }
        let mut lines: Vec<_> = self.events.drain(..).collect();
        lines.sort_by_key(|line| line.seq);
        lines.dedup_by_key(|line| line.seq);
        for line in lines {
            self.events.push_back(line);
        }
        while self.events.len() > 2000 {
            self.events.pop_front();
        }
    }

    /// Selects the newest goal once, so the detail panel is useful on the first frame.
    fn select_initial_goal(&mut self) {
        if self.auto_selected || self.selected_goal.is_some() {
            return;
        }
        let Some(project) = self.selected_project.as_deref() else {
            return;
        };
        let newest = self
            .goals
            .iter()
            .filter(|goal| goal.project_id == project)
            .max_by(|a, b| a.created_at.cmp(&b.created_at))
            .map(|goal| goal.id.clone());
        if let Some(id) = newest {
            self.selected_goal = Some(id);
            self.auto_selected = true;
        }
    }

    fn reconcile_selection(&mut self) {
        if let Some(id) = &self.selected_project {
            if !self.projects.iter().any(|project| &project.id == id) {
                self.selected_project = None;
            }
        }
        if self.selected_project.is_none() {
            self.selected_project = self.projects.first().map(|project| project.id.clone());
        }

        if let Some(id) = &self.selected_goal {
            if !self.goals.iter().any(|goal| &goal.id == id) {
                self.selected_goal = None;
                self.selected_task = None;
                self.run_detail = None;
            }
        }
        if let Some(id) = &self.selected_task {
            if !self.tasks.iter().any(|task| &task.id == id) {
                self.selected_task = None;
            }
        }
    }

    pub fn project(&self) -> Option<&ProjectView> {
        let id = self.selected_project.as_deref()?;
        self.projects.iter().find(|project| project.id == id)
    }

    pub fn project_by_id(&self, id: &str) -> Option<&ProjectView> {
        self.projects.iter().find(|project| project.id == id)
    }

    /// Goals of the selected project, newest first.
    pub fn goals(&self) -> Vec<&jcs_proto::GoalView> {
        let Some(project) = self.project() else {
            return Vec::new();
        };
        let mut goals: Vec<_> = self
            .goals
            .iter()
            .filter(|goal| goal.project_id == project.id)
            .collect();
        goals.sort_by_key(|goal| std::cmp::Reverse(goal.created_at));
        goals
    }

    pub fn selected_goal(&self) -> Option<&jcs_proto::GoalView> {
        let id = self.selected_goal.as_deref()?;
        self.goals.iter().find(|goal| goal.id == id)
    }

    pub fn selected_task(&self) -> Option<&TaskView> {
        let id = self.selected_task.as_deref()?;
        self.tasks.iter().find(|task| task.id == id)
    }

    /// Tokens consumed by all attempts of a task.
    pub fn task_tokens(&self, task_id: &str) -> i64 {
        self.runs
            .iter()
            .filter(|run| run.task_id == task_id)
            .map(|run| run.tokens_in + run.tokens_out)
            .sum()
    }

    /// Seconds since the task was created, or until it reached a terminal state.
    pub fn task_elapsed_secs(&self, task: &TaskView) -> i64 {
        let end = if matches!(task.status.as_str(), "succeeded" | "failed" | "cancelled") {
            task.updated_at.as_second()
        } else {
            jiff::Timestamp::now().as_second()
        };
        (end - task.created_at.as_second()).max(0)
    }

    /// Why a pending task is not running yet, in the operator's terms. This mirrors the
    /// scheduler's rules (dependencies, path claims, capacity) without duplicating state.
    pub fn task_wait_reason(&self, task: &TaskView) -> Option<String> {
        if task.status != "pending" {
            return None;
        }

        let mut waiting = Vec::new();
        for dependency in &task.depends_on {
            let Some(other) = self.tasks.iter().find(|other| &other.id == dependency) else {
                waiting.push(crate::daemon::short(dependency).to_string());
                continue;
            };
            if other.status != "succeeded" {
                waiting.push(format!(
                    "{} ({})",
                    crate::daemon::short(&other.id),
                    other.status
                ));
            } else if other.merged_at.is_none() {
                waiting.push(format!(
                    "integración de {}",
                    crate::daemon::short(&other.id)
                ));
            }
        }
        if !waiting.is_empty() {
            return Some(format!("espera {}", waiting.join(", ")));
        }

        let project_id = self
            .goals
            .iter()
            .find(|goal| goal.id == task.goal_id)
            .map(|goal| goal.project_id.clone());
        let same_project = |other: &TaskView| {
            self.goals
                .iter()
                .find(|goal| goal.id == other.goal_id)
                .is_some_and(|goal| Some(&goal.project_id) == project_id.as_ref())
        };

        for other in self.tasks.iter().filter(|other| {
            matches!(other.status.as_str(), "running" | "verifying") && same_project(other)
        }) {
            if jcs_core::claims::claims_overlap(&task.path_claims, &other.path_claims) {
                return Some(format!(
                    "espera: {} trabaja en las mismas rutas",
                    crate::daemon::short(&other.id)
                ));
            }
        }

        let running = self
            .tasks
            .iter()
            .filter(|other| {
                matches!(other.status.as_str(), "running" | "verifying") && same_project(other)
            })
            .count();
        if let Some(settings) = &self.settings {
            if running >= settings.max_concurrent as usize {
                return Some(format!(
                    "espera turno ({running}/{} en curso)",
                    settings.max_concurrent
                ));
            }
        }

        None
    }

    pub fn goal_tasks(&self, goal_id: &str) -> Vec<&jcs_proto::TaskView> {
        self.tasks
            .iter()
            .filter(|task| task.goal_id == goal_id)
            .collect()
    }

    pub fn task_runs(&self, task_id: &str) -> Vec<&jcs_proto::RunView> {
        let mut runs: Vec<_> = self
            .runs
            .iter()
            .filter(|run| run.task_id == task_id)
            .collect();
        runs.sort_by_key(|run| run.started_at);
        runs
    }

    /// Total spend of a goal, derived from its runs. Never estimated.
    pub fn goal_cost(&self, goal_id: &str) -> (f64, i64, i64) {
        let mut cost = 0.0;
        let mut tokens_in = 0;
        let mut tokens_out = 0;
        for task in self.tasks.iter().filter(|task| task.goal_id == goal_id) {
            for run in self.runs.iter().filter(|run| run.task_id == task.id) {
                cost += run.cost_usd;
                tokens_in += run.tokens_in;
                tokens_out += run.tokens_out;
            }
        }
        (cost, tokens_in, tokens_out)
    }

    /// The run that is still going, if any. Used to offer "abort".
    pub fn running_run(&self, goal_id: &str) -> Option<&jcs_proto::RunView> {
        self.tasks
            .iter()
            .filter(|task| task.goal_id == goal_id)
            .flat_map(|task| self.runs.iter().filter(move |run| run.task_id == task.id))
            .find(|run| run.status == "started")
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.connection, Some(Connection::Online))
    }
}

/// One-line description of an event, bounded so the console cannot grow fat.
fn summarize(event: &Event) -> String {
    // Agent text is the trace itself; everything else is a summary. The console truncates
    // for display, so keeping the full chunk here does not flood the window.
    let limit = match event.kind.as_str() {
        "agent_text" | "agent_reasoning" => 4_000,
        _ => 220,
    };

    let payload = &event.payload;
    let text = match event.kind.as_str() {
        "agent_text" | "agent_reasoning" => payload
            .get("text")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
        "agent_tool_called" | "agent_tool_finished" => {
            let tool = payload
                .get("tool")
                .and_then(|value| value.as_str())
                .unwrap_or("herramienta");
            let detail = payload
                .get("output")
                .or_else(|| payload.get("input"))
                .map(|value| match value {
                    serde_json::Value::String(text) => text.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_default();
            format!("{tool} · {}", detail.trim())
        }
        "error" => {
            let reason = payload
                .get("reason")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let hint = payload
                .get("hint")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if hint.is_empty() {
                reason.to_string()
            } else {
                format!("{reason} · {hint}")
            }
        }
        _ => {
            let mut parts = Vec::new();
            if let Some(object) = payload.as_object() {
                for (key, value) in object {
                    let rendered = match value {
                        serde_json::Value::String(text) => text.clone(),
                        other => other.to_string(),
                    };
                    parts.push(format!("{key}={rendered}"));
                }
            }
            parts.join(" · ")
        }
    };

    if text.chars().count() > limit {
        let mut short: String = text.chars().take(limit).collect();
        short.push('…');
        short
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jcs_core::{EventId, EventKind};
    use std::str::FromStr;

    fn event(seq: i64, kind: EventKind, payload: serde_json::Value) -> Event {
        Event {
            seq,
            id: EventId::from_str(&format!("evt_{seq:032x}")).unwrap(),
            ts: jiff::Timestamp::now(),
            kind,
            project_id: None,
            goal_id: None,
            task_id: None,
            run_id: None,
            payload,
        }
    }

    #[test]
    fn the_event_console_is_bounded() {
        let mut state = State::default();
        for seq in 1..=2200 {
            state.push_event(event(seq, EventKind::AgentText, serde_json::json!({})));
        }
        assert_eq!(state.events.len(), 2000);
        assert_eq!(state.events.front().unwrap().seq, 201);
    }

    #[test]
    fn replayed_history_does_not_duplicate_lines() {
        let mut state = State::default();
        state.push_event(event(7, EventKind::DaemonStarted, serde_json::json!({})));
        state.push_event(event(7, EventKind::DaemonStarted, serde_json::json!({})));
        assert_eq!(state.events.len(), 1);
    }

    #[test]
    fn events_keep_the_goal_they_belong_to() {
        let mut event = event(3, EventKind::RunStarted, serde_json::json!({}));
        event.goal_id = Some(jcs_core::GoalId::new());
        let expected = event.goal_id.as_ref().unwrap().to_string();
        let mut state = State::default();
        state.push_event(event);
        assert_eq!(state.events[0].goal_id.as_deref(), Some(expected.as_str()));
    }

    #[test]
    fn errors_show_reason_and_hint() {
        let event = event(
            1,
            EventKind::Error,
            serde_json::json!({"reason": "budget exhausted", "hint": "raise it"}),
        );
        assert_eq!(summarize(&event), "budget exhausted · raise it");
    }
}
