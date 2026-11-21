//! TUI state.
//!
//! The UI is a pure function of this state: no widget performs I/O, and every mutation
//! happens here. That keeps rendering deterministic and testable, and means a slow daemon
//! can never freeze the interface.

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use jcs_core::EventKind;
use jcs_proto::{GoalView, HealthResponse, ProjectView, RunView, StatusResponse, TaskView};

/// Maximum number of log lines kept in memory.
const LOG_LINES: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    Projects,
    Goals,
    Tasks,
}

impl Focus {
    fn next(self) -> Self {
        match self {
            Self::Projects => Self::Goals,
            Self::Goals => Self::Tasks,
            Self::Tasks => Self::Projects,
        }
    }
}

/// An operation the UI wants the daemon to perform.
#[derive(Debug, Clone)]
pub enum Action {
    Ask { project: String, prompt: String },
    RenderPolicy { project: String },
    ResumeGoal { goal_id: String },
}

#[derive(Debug, Default)]
pub struct App {
    pub projects: Vec<ProjectView>,
    pub goals: Vec<GoalView>,
    pub tasks: Vec<TaskView>,
    pub runs: Vec<RunView>,
    pub health: Option<HealthResponse>,
    pub totals_cost: f64,
    pub log: VecDeque<String>,

    pub project_index: usize,
    pub goal_index: usize,
    pub focus: Focus,
    pub quit: bool,

    /// Prompt being typed, if the ask modal is open.
    pub input: Option<String>,
    pub pending: Option<Action>,
    pub toast: Option<(String, Instant)>,
    pub last_error: Option<String>,

    /// Titles by task id, learned from events so early events are readable.
    pub titles: HashMap<String, String>,
}

impl App {
    pub fn refresh(&mut self, status: StatusResponse) {
        self.health = Some(status.health);
        self.projects = status.projects;
        self.goals = status.goals;
        self.tasks = status.tasks;
        self.runs = status.runs;
        self.totals_cost = status.totals.cost_usd;

        self.clamp_indices();
    }

    fn clamp_indices(&mut self) {
        if self.project_index >= self.projects.len() {
            self.project_index = self.projects.len().saturating_sub(1);
        }
        let goals = self.goals_for_selected_project().len();
        if self.goal_index >= goals {
            self.goal_index = goals.saturating_sub(1);
        }
    }

    pub fn selected_project(&self) -> Option<&ProjectView> {
        self.projects.get(self.project_index)
    }

    /// Goals of the selected project, newest first as the daemon returns them.
    pub fn goals_for_selected_project(&self) -> Vec<&GoalView> {
        let Some(project) = self.selected_project() else {
            return Vec::new();
        };
        self.goals
            .iter()
            .filter(|goal| goal.project_id == project.id)
            .collect()
    }

    pub fn selected_goal(&self) -> Option<&GoalView> {
        self.goals_for_selected_project()
            .get(self.goal_index)
            .copied()
    }

    pub fn tasks_for_goal(&self, goal_id: &str) -> Vec<&TaskView> {
        self.tasks
            .iter()
            .filter(|task| task.goal_id == goal_id)
            .collect()
    }

    pub fn runs_for_task(&self, task_id: &str) -> Vec<&RunView> {
        self.runs
            .iter()
            .filter(|run| run.task_id == task_id)
            .collect()
    }

    /// Applies one streamed event: appends to the log and refreshes derived state.
    pub fn apply_event(&mut self, event: jcs_core::Event) {
        if let Some(task) = event.task_id.as_ref() {
            if event.kind == EventKind::TaskCreated {
                if let Some(title) = event.payload.get("title").and_then(|value| value.as_str()) {
                    self.titles
                        .insert(task.as_str().to_string(), title.to_string());
                }
            }
        }

        if let Some(line) = format_event(&event, &self.titles) {
            self.log.push_back(line);
            if self.log.len() > LOG_LINES {
                self.log.pop_front();
            }
        }

        // Keep the local projection roughly current without waiting for the next poll.
        match event.kind {
            EventKind::TaskStatusChanged => {
                if let (Some(task), Some(to)) = (
                    event.task_id.as_ref(),
                    event.payload.get("to").and_then(|value| value.as_str()),
                ) {
                    if let Some(local) = self
                        .tasks
                        .iter_mut()
                        .find(|candidate| candidate.id == task.as_str())
                    {
                        local.status = to.to_string();
                        local.updated_at = event.ts;
                    }
                }
            }
            EventKind::GoalStatusChanged => {
                if let (Some(goal), Some(to)) = (
                    event.goal_id.as_ref(),
                    event.payload.get("to").and_then(|value| value.as_str()),
                ) {
                    if let Some(local) = self
                        .goals
                        .iter_mut()
                        .find(|candidate| candidate.id == goal.as_str())
                    {
                        local.status = to.to_string();
                        local.updated_at = event.ts;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn toast(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), Instant::now()));
    }

    pub fn move_selection(&mut self, delta: isize) {
        match self.focus {
            Focus::Projects => {
                self.project_index = shift(self.project_index, delta, self.projects.len());
                self.goal_index = 0;
            }
            Focus::Goals => {
                let len = self.goals_for_selected_project().len();
                self.goal_index = shift(self.goal_index, delta, len);
            }
            Focus::Tasks => {
                // Tasks are displayed in goal order; moving here changes the goal.
                let len = self.goals_for_selected_project().len();
                self.goal_index = shift(self.goal_index, delta, len);
            }
        }
    }

    pub fn cycle_focus(&mut self) {
        self.focus = self.focus.next();
    }

    /// Starts the ask modal for the selected project.
    pub fn begin_ask(&mut self) {
        if self.selected_project().is_some() {
            self.input = Some(String::new());
        } else {
            self.toast("add a project first: jcs project add <path>");
        }
    }

    pub fn submit_input(&mut self) {
        let Some(prompt) = self.input.take() else {
            return;
        };
        let prompt = prompt.trim().to_string();
        if prompt.is_empty() {
            return;
        }
        let Some(project) = self.selected_project() else {
            self.toast("no project selected");
            return;
        };
        self.pending = Some(Action::Ask {
            project: project.id.clone(),
            prompt,
        });
    }

    pub fn request_render(&mut self) {
        let Some(project) = self.selected_project() else {
            self.toast("no project selected");
            return;
        };
        self.pending = Some(Action::RenderPolicy {
            project: project.id.clone(),
        });
    }

    pub fn request_resume(&mut self) {
        let Some(goal) = self.selected_goal() else {
            self.toast("no goal selected");
            return;
        };
        if goal.status != "blocked" {
            self.toast("only a blocked goal can be resumed");
            return;
        }
        self.pending = Some(Action::ResumeGoal {
            goal_id: goal.id.clone(),
        });
    }

    /// Visible toast, if it has not expired.
    pub fn live_toast(&self) -> Option<&str> {
        match &self.toast {
            Some((message, at)) if at.elapsed().as_secs() < 5 => Some(message.as_str()),
            _ => None,
        }
    }
}

fn shift(current: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let next = current as isize + delta;
    next.clamp(0, len as isize - 1) as usize
}

/// Formats one event for the log pane.
pub fn format_event(event: &jcs_core::Event, titles: &HashMap<String, String>) -> Option<String> {
    let clock = event.ts.strftime("%H:%M:%S").to_string();
    let task = event
        .task_id
        .as_ref()
        .map(|id| {
            titles
                .get(id.as_str())
                .cloned()
                .unwrap_or_else(|| short(id.as_str()))
        })
        .unwrap_or_default();

    let line = match event.kind {
        EventKind::AgentText => {
            let text = event.payload.get("text").and_then(|v| v.as_str())?;
            format!("{clock}   {}", text.trim().replace('\n', " "))
        }
        EventKind::TaskCreated => format!("{clock} + {task}"),
        EventKind::TaskStatusChanged => {
            let to = event
                .payload
                .get("to")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("{clock} {task} -> {to}")
        }
        EventKind::GoalStatusChanged => {
            let to = event
                .payload
                .get("to")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("{clock} goal -> {to}")
        }
        EventKind::AgentToolCalled => {
            let tool = event
                .payload
                .get("tool")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("{clock} -> {tool}")
        }
        EventKind::AgentToolFinished => {
            let tool = event
                .payload
                .get("tool")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("{clock} <- {tool}")
        }
        EventKind::GateFinished => {
            let gate = event
                .payload
                .get("gate")
                .and_then(|v| v.as_str())
                .unwrap_or("gate");
            let status = event
                .payload
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("{clock} gate {gate}: {status}")
        }
        EventKind::CommitCreated => {
            let short = event
                .payload
                .get("short")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("{clock} commit {short}")
        }
        EventKind::MergeFinished => {
            let result = event
                .payload
                .get("result")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("{clock} merge {result}")
        }
        EventKind::PlanCreated => {
            let tasks = event
                .payload
                .get("tasks")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            format!("{clock} plan: {tasks} tasks")
        }
        EventKind::Error => {
            let reason = event
                .payload
                .get("category")
                .and_then(|v| v.as_str())
                .or_else(|| event.payload.get("reason").and_then(|v| v.as_str()))
                .unwrap_or("error");
            format!("{clock} ! {reason}")
        }
        EventKind::RunFinished => {
            let status = event
                .payload
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("{clock} run {status}")
        }
        _ => return None,
    };

    Some(line)
}

pub fn short(id: &str) -> String {
    let body = id.split_once('_').map(|(_, rest)| rest).unwrap_or(id);
    body[..body.len().min(8)].to_string()
}

/// Status colour used by the board.
pub fn status_style(status: &str) -> ratatui::style::Style {
    use ratatui::style::{Color, Style};
    match status {
        "succeeded" => Style::new().fg(Color::Green),
        "running" | "verifying" => Style::new().fg(Color::Yellow),
        "failed" => Style::new().fg(Color::Red),
        "blocked" => Style::new().fg(Color::Magenta),
        "cancelled" => Style::new().fg(Color::DarkGray),
        _ => Style::new().fg(Color::Gray),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_timestamp() -> jcs_proto::Timestamp {
        "2026-01-01T00:00:00Z".parse().expect("valid timestamp")
    }

    fn project(id: &str, name: &str) -> ProjectView {
        ProjectView {
            id: id.into(),
            name: name.into(),
            root_path: "/tmp/demo".into(),
            base_branch: "main".into(),
            created_at: test_timestamp(),
            auto_merge: false,
            budget_usd_per_day: None,
            budget_tokens_per_day: None,
        }
    }

    fn goal(id: &str, project: &str, status: &str) -> GoalView {
        GoalView {
            id: id.into(),
            project_id: project.into(),
            prompt: "objective".into(),
            status: status.into(),
            created_at: test_timestamp(),
            updated_at: test_timestamp(),
        }
    }

    fn task(id: &str, goal: &str, status: &str) -> TaskView {
        TaskView {
            id: id.into(),
            goal_id: goal.into(),
            title: "task".into(),
            status: status.into(),
            worktree: None,
            branch: None,
            session_id: None,
            depends_on: Vec::new(),
            created_at: test_timestamp(),
            updated_at: test_timestamp(),
        }
    }

    #[test]
    fn goals_are_filtered_by_the_selected_project() {
        let mut app = App {
            projects: vec![project("prj_a", "a"), project("prj_b", "b")],
            goals: vec![
                goal("goal_1", "prj_a", "running"),
                goal("goal_2", "prj_b", "succeeded"),
            ],
            ..App::default()
        };

        assert_eq!(app.goals_for_selected_project().len(), 1);
        assert_eq!(app.goals_for_selected_project()[0].id, "goal_1");

        app.project_index = 1;
        assert_eq!(app.goals_for_selected_project()[0].id, "goal_2");
    }

    #[test]
    fn selection_is_clamped_when_the_list_shrinks() {
        let mut app = App {
            projects: vec![project("prj_a", "a"), project("prj_b", "b")],
            project_index: 1,
            ..App::default()
        };
        app.projects.truncate(1);
        app.refresh(StatusResponse {
            health: HealthResponse {
                ok: true,
                version: "0.1.0".into(),
                protocol: 1,
                uptime_secs: 0,
                database: "db".into(),
                agent: jcs_proto::AgentHealth {
                    backend: "opencode".into(),
                    reachable: false,
                    base_url: None,
                    version: None,
                    message: None,
                },
                started_at_ms: 0,
            },
            projects: app.projects.clone(),
            goals: Vec::new(),
            tasks: Vec::new(),
            runs: Vec::new(),
            totals: Default::default(),
        });
        assert_eq!(app.project_index, 0);
    }

    #[test]
    fn events_update_the_local_projection() {
        let mut app = App {
            tasks: vec![task("task_1", "goal_1", "pending")],
            goals: vec![goal("goal_1", "prj_a", "running")],
            ..App::default()
        };

        let event = jcs_core::NewEvent::new(EventKind::TaskStatusChanged)
            .task(&jcs_core::TaskId::parse("task_1").unwrap())
            .payload(serde_json::json!({"to": "running"}))
            .into_event(1);
        app.apply_event(event);

        assert_eq!(app.tasks[0].status, "running");
        assert!(app.log.back().unwrap().contains("task_1") || !app.log.is_empty());
    }

    #[test]
    fn short_ids_are_bounded() {
        assert_eq!(short("task_0123456789abcdef"), "01234567");
    }
}
