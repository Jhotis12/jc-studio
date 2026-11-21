//! JC Studio desktop application.
//!
//! An installable, agent-oriented control plane: projects, objectives, the task DAG,
//! gates, memory, settings and embedded terminals. The daemon owns all durable state; this
//! window is a client, so closing it cannot lose work.

use std::time::Duration;

use crate::daemon::{Command, Daemon};
use crate::state::State;
use jcs_proto::paths;

/// Which dock is visible at the bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConsoleTab {
    #[default]
    Events,
    Agent,
    Terminals,
}

/// Which section of the project workspace is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    Orchestrator,
    #[default]
    Goals,
    Code,
    Details,
    Extensions,
    Policy,
    Memory,
    System,
}

/// Editable copy of the project execution settings. Only sent when the operator saves.
#[derive(Debug, Clone, Default)]
pub struct ProjectForm {
    pub project: Option<String>,
    pub auto_merge: bool,
    pub budget_usd: String,
    pub budget_tokens: String,
}

/// Editable global settings. Text fields stay strings so a half-typed value is not lost.
#[derive(Debug, Clone)]
pub struct GlobalForm {
    pub default_model: String,
    pub default_agent: String,
    pub planning_model: String,
    pub planning_agent: String,
    pub review_model: String,
    pub review: bool,
    pub risk_max_lines: String,
    pub run_timeout_secs: String,
    pub context_tokens: String,
    pub max_repair_cycles: String,
    pub max_concurrent: String,
    pub telegram_token: String,
    pub telegram_chat_ids: String,
    pub clear_telegram_token: bool,
}

impl Default for GlobalForm {
    fn default() -> Self {
        Self {
            default_model: String::new(),
            default_agent: String::new(),
            planning_model: String::new(),
            planning_agent: String::new(),
            review_model: String::new(),
            review: false,
            risk_max_lines: "400".to_string(),
            run_timeout_secs: "900".to_string(),
            context_tokens: "1500".to_string(),
            max_repair_cycles: "2".to_string(),
            max_concurrent: "4".to_string(),
            telegram_token: String::new(),
            telegram_chat_ids: String::new(),
            clear_telegram_token: false,
        }
    }
}

impl GlobalForm {
    pub fn from_view(view: &jcs_proto::SettingsView) -> Self {
        Self {
            default_model: view.default_model.clone().unwrap_or_default(),
            default_agent: view.default_agent.clone().unwrap_or_default(),
            planning_model: view.planning_model.clone().unwrap_or_default(),
            planning_agent: view.planning_agent.clone().unwrap_or_default(),
            review_model: view.review_model.clone().unwrap_or_default(),
            review: view.review,
            risk_max_lines: view.risk_max_lines.to_string(),
            run_timeout_secs: view.run_timeout_secs.to_string(),
            context_tokens: view.context_tokens.to_string(),
            max_repair_cycles: view.max_repair_cycles.to_string(),
            max_concurrent: view.max_concurrent.to_string(),
            telegram_token: String::new(),
            telegram_chat_ids: view
                .telegram_chat_ids
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            clear_telegram_token: false,
        }
    }

    pub fn patch(&self) -> Result<jcs_proto::SettingsPatch, String> {
        Ok(jcs_proto::SettingsPatch {
            default_model: Some(self.default_model.trim().to_string()),
            default_agent: Some(self.default_agent.trim().to_string()),
            planning_model: Some(self.planning_model.trim().to_string()),
            planning_agent: Some(self.planning_agent.trim().to_string()),
            review_model: Some(self.review_model.trim().to_string()),
            review: Some(self.review),
            risk_max_lines: Some(number(&self.risk_max_lines, "el techo de líneas")?),
            run_timeout_secs: Some(number(&self.run_timeout_secs, "el timeout")?),
            context_tokens: Some(number(&self.context_tokens, "el contexto")?),
            max_repair_cycles: Some(number(&self.max_repair_cycles, "los ciclos de reparación")?),
            max_concurrent: Some(number(&self.max_concurrent, "la concurrencia")?),
            telegram_token: if self.clear_telegram_token {
                Some(String::new())
            } else {
                let token = self.telegram_token.trim();
                (!token.is_empty()).then(|| token.to_string())
            },
            telegram_chat_ids: Some(parse_chat_ids(&self.telegram_chat_ids)?),
            agent_skills: None,
        })
    }
}

/// Accepts commas, spaces or both between chat ids.
fn parse_chat_ids(value: &str) -> Result<Vec<i64>, String> {
    let mut ids = Vec::new();
    for part in value.split(|character: char| character == ',' || character.is_whitespace()) {
        if part.is_empty() {
            continue;
        }
        let id = part
            .parse::<i64>()
            .map_err(|_| format!("chat id inválido: {part}"))?;
        if id == 0 {
            return Err("los chat ids no pueden ser cero".to_string());
        }
        ids.push(id);
    }
    if ids.len() > 20 {
        return Err("como máximo 20 chats".to_string());
    }
    Ok(ids)
}

fn number(value: &str, label: &str) -> Result<u64, String> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("{label} debe ser un número entero"))
}

/// Fields for registering a repository. Kept as strings: the daemon validates, not the UI.
#[derive(Debug, Default)]
pub struct AddProjectForm {
    pub path: String,
    pub name: String,
    pub base_branch: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtensionsTab {
    #[default]
    Skills,
    Mcp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum McpKind {
    #[default]
    Local,
    Remote,
}

#[derive(Debug, Clone, Default)]
pub struct McpForm {
    pub name: String,
    pub kind: McpKind,
    pub command: String,
    pub url: String,
}

pub struct Studio {
    pub state: State,
    pub daemon: Daemon,

    pub tab: Tab,
    pub objective: String,
    pub plan: bool,
    pub model: String,
    pub event_filter: String,
    pub events_for_goal: bool,
    pub show_console: bool,
    pub console_tab: ConsoleTab,
    pub show_settings: bool,
    pub show_add_project: bool,
    pub add_project: AddProjectForm,
    pub project_form: ProjectForm,
    pub global_form: GlobalForm,
    pub settings_synced: bool,
    pub confirm_remove: bool,
    pub confirm_restart: bool,
    pub confirm_delete_goal: bool,
    pub memory_query: String,
    /// Whether the window is wide enough for the three-panel layout.
    pub wide: bool,

    /// Project whose policy and memory are already loaded, to avoid a request per frame.
    loaded_project: Option<String>,
    /// Task whose historical trace was already requested.
    loaded_task_events: Option<String>,
    /// Goal for which a task was selected automatically once.
    auto_task_for: Option<String>,
    /// Project whose skills/MCP catalog is loaded.
    pub extensions_loaded_for: Option<String>,
    pub mcp_form: McpForm,
    pub extensions_tab: ExtensionsTab,
    pub skill_filter: String,
    pub goal_filter: String,
    pub show_finished_goals: bool,
    pub iteration_for: Option<String>,
    pub iteration_feedback: String,
    pub orchestrator_input: String,
    pub orchestrator_loaded_for: Option<String>,
    /// Assistant reply being turned into an objective, with the operator's edits.
    pub orchestrator_goal: Option<String>,
    pub orchestrator_goal_plan: bool,
    /// Terminal sessions, one per open tab.
    pub terminals: crate::terminal::Terminals,
    /// File tree, editor and task diff.
    pub code: crate::ui::code::CodeState,
}

impl Studio {
    pub fn new() -> Self {
        Self {
            state: State {
                connection: Some(crate::state::Connection::Connecting),
                ..State::default()
            },
            daemon: Daemon::spawn(paths::socket_path()),
            tab: Tab::default(),
            objective: String::new(),
            plan: true,
            model: String::new(),
            event_filter: String::new(),
            events_for_goal: false,
            show_console: true,
            console_tab: ConsoleTab::default(),
            show_settings: false,
            show_add_project: false,
            add_project: AddProjectForm::default(),
            project_form: ProjectForm::default(),
            global_form: GlobalForm::default(),
            settings_synced: false,
            confirm_remove: false,
            confirm_restart: false,
            confirm_delete_goal: false,
            memory_query: String::new(),
            wide: true,
            loaded_project: None,
            loaded_task_events: None,
            auto_task_for: None,
            extensions_loaded_for: None,
            mcp_form: McpForm::default(),
            extensions_tab: ExtensionsTab::default(),
            skill_filter: String::new(),
            goal_filter: String::new(),
            show_finished_goals: true,
            iteration_for: None,
            iteration_feedback: String::new(),
            orchestrator_input: String::new(),
            orchestrator_loaded_for: None,
            orchestrator_goal: None,
            orchestrator_goal_plan: true,
            terminals: crate::terminal::Terminals::default(),
            code: crate::ui::code::CodeState::default(),
        }
    }

    pub fn select_project(&mut self, id: String) {
        self.state.selected_project = Some(id);
        self.orchestrator_loaded_for = None;
        self.state.selected_goal = None;
        self.state.selected_task = None;
        self.state.auto_selected = false;
        self.state.run_detail = None;
    }

    /// Sends a conversational turn. The user's message appears immediately; the reply
    /// arrives through the daemon channel and replaces the transcript.
    pub fn send_orchestrator(&mut self) {
        let prompt = self.orchestrator_input.trim().to_string();
        if prompt.is_empty() || self.state.orchestrator_busy {
            return;
        }
        let Some(project) = self.state.project().map(|project| project.name.clone()) else {
            return;
        };
        self.state
            .orchestrator
            .push(jcs_proto::OrchestratorMessage {
                role: "user".to_string(),
                text: prompt.clone(),
                created_at: jiff::Timestamp::now(),
            });
        self.state.orchestrator_busy = true;
        self.orchestrator_input.clear();
        self.daemon
            .send(Command::SendOrchestrator { project, prompt });
    }

    pub fn select_goal(&mut self, id: String) {
        self.state.selected_goal = Some(id);
        self.state.selected_task = None;
        self.auto_task_for = None;
        self.state.run_detail = None;
        self.confirm_delete_goal = false;
        // On a narrow window the detail is a tab, so selecting a goal must reveal it.
        if !self.wide {
            self.tab = Tab::Details;
        }
    }

    /// Selects a task and loads its historical trace once. Live events keep it current.
    /// Selects a task and brings its live trace to the front. Used by the graph and the
    /// per-task buttons, so "how is this task going" is one click away.
    pub fn focus_task(&mut self, goal: String, task: String) {
        self.select_goal(goal);
        self.select_task(task);
        self.console_tab = ConsoleTab::Agent;
        self.show_console = true;
        if !self.wide {
            self.tab = Tab::Details;
        }
    }

    pub fn select_task(&mut self, id: String) {
        self.state.selected_task = Some(id.clone());
        if self.loaded_task_events.as_deref() != Some(id.as_str()) {
            self.loaded_task_events = Some(id.clone());
            self.daemon.send(Command::TaskEvents { task: id });
        }
    }

    /// Opens the Code tab on a task's worktree with its diff already loaded.
    pub fn show_task_changes(&mut self, goal: String, task: String) {
        self.select_goal(goal);
        self.select_task(task);
        self.code.prefer_worktree = true;
        self.code.pending_diff = true;
        self.tab = Tab::Code;
    }

    /// Keyboard navigation that matters while working: switch workspace, settings, console.
    fn shortcuts(&mut self, context: &egui::Context) {
        let pressed =
            |key| context.input(|input| input.modifiers.command && input.key_pressed(key));
        if pressed(egui::Key::Num1) {
            self.tab = Tab::Goals;
        } else if pressed(egui::Key::Num2) {
            self.tab = Tab::Code;
        } else if pressed(egui::Key::Num3) {
            self.tab = Tab::Policy;
        } else if pressed(egui::Key::Num4) {
            self.tab = Tab::Memory;
        } else if pressed(egui::Key::Num5) {
            self.tab = Tab::System;
        }
        if pressed(egui::Key::E) {
            self.tab = Tab::Extensions;
        }
        if pressed(egui::Key::O) {
            self.tab = Tab::Orchestrator;
        }
        if pressed(egui::Key::Comma) {
            self.open_settings();
        }
        if pressed(egui::Key::J) {
            self.show_console = !self.show_console;
        }
    }

    /// Picks the active task of a selected goal once, so the agent trace has a subject
    /// without an extra click. Never fights an explicit selection.
    fn sync_task(&mut self) {
        let Some(goal_id) = self.state.selected_goal.clone() else {
            return;
        };
        if self.state.selected_task.is_some()
            || self.auto_task_for.as_deref() == Some(goal_id.as_str())
        {
            return;
        }
        self.auto_task_for = Some(goal_id.clone());
        let tasks = self.state.goal_tasks(&goal_id);
        let chosen = tasks
            .iter()
            .find(|task| matches!(task.status.as_str(), "running" | "verifying"))
            .or_else(|| tasks.iter().find(|task| task.status == "pending"))
            .or_else(|| tasks.first())
            .map(|task| task.id.clone());
        if let Some(id) = chosen {
            self.select_task(id);
        }
    }

    /// Loads what is project-scoped when the selection changes, exactly once per change.
    fn sync_project(&mut self) {
        if self.loaded_project == self.state.selected_project {
            return;
        }
        self.loaded_project = self.state.selected_project.clone();
        self.state.policy = None;
        self.state.rendered = None;
        self.state.gates = None;
        self.state.memory = None;
        self.state.memory_index = None;
        self.state.memory_search = None;
        self.state.run_detail = None;

        let Some(project) = self.state.selected_project.clone() else {
            return;
        };
        let Some(view) = self.state.project_by_id(&project).cloned() else {
            // The snapshot that contains the project has not arrived yet. Leave the load
            // pending so the next refresh retries instead of caching an error.
            self.loaded_project = None;
            return;
        };
        self.daemon.send(Command::Policy {
            project: view.name.clone(),
        });
        self.daemon
            .send(Command::MemoryStats { project: view.name });
    }

    /// Fills the settings form from the project, unless the operator is already editing it.
    pub fn sync_project_form(&mut self) {
        let Some(project) = self.state.project() else {
            self.project_form = ProjectForm::default();
            return;
        };
        if self.project_form.project.as_deref() == Some(project.id.as_str()) {
            return;
        }
        self.project_form = ProjectForm {
            project: Some(project.id.clone()),
            auto_merge: project.auto_merge,
            budget_usd: project
                .budget_usd_per_day
                .map(format_amount)
                .unwrap_or_default(),
            budget_tokens: project
                .budget_tokens_per_day
                .map(|tokens| tokens.to_string())
                .unwrap_or_default(),
        };
        self.confirm_remove = false;
    }

    /// Re-queues a failed or cancelled objective with the same prompt as a new goal.
    pub fn retry_goal(&mut self, goal_id: &str) {
        let Some(goal) = self.state.goals.iter().find(|goal| goal.id == goal_id) else {
            return;
        };
        let prompt = goal.prompt.clone();
        let Some(project) = self
            .state
            .project_by_id(&goal.project_id)
            .map(|project| project.name.clone())
        else {
            return;
        };
        self.daemon.send(Command::Ask {
            project,
            prompt,
            model: None,
            plan: true,
        });
        self.state.notice = Some(crate::state::Notice {
            message: "objetivo reencolado como uno nuevo".to_string(),
            error: false,
        });
    }

    /// Enables or disables a skill for future task sessions. Applied immediately; the
    /// daemon activates the selection when it creates a session.
    pub fn toggle_skill(&mut self, id: String, enabled: bool) {
        let Some(settings) = &self.state.settings else {
            return;
        };
        let mut skills = settings.agent_skills.clone();
        if enabled {
            if !skills.contains(&id) {
                skills.push(id);
            }
        } else {
            skills.retain(|skill| skill != &id);
        }
        self.daemon
            .send(Command::SaveSettings(jcs_proto::SettingsPatch {
                agent_skills: Some(skills),
                ..Default::default()
            }));
    }

    /// Adds an MCP server at runtime. The daemon validates the config and the backend
    /// connects it immediately; persistence is OpenCode's responsibility.
    pub fn add_mcp(&mut self) {
        let Some(project) = self.state.project() else {
            return;
        };
        let name = self.mcp_form.name.trim().to_string();
        if name.is_empty() {
            self.state.notice = Some(crate::state::Notice {
                message: "el servidor MCP necesita un nombre".to_string(),
                error: true,
            });
            return;
        }
        let config = match self.mcp_form.kind {
            McpKind::Local => {
                let command: Vec<String> = self
                    .mcp_form
                    .command
                    .split_whitespace()
                    .map(str::to_string)
                    .collect();
                if command.is_empty() {
                    self.state.notice = Some(crate::state::Notice {
                        message: "un MCP local necesita un comando".to_string(),
                        error: true,
                    });
                    return;
                }
                serde_json::json!({ "type": "local", "command": command })
            }
            McpKind::Remote => {
                let url = self.mcp_form.url.trim();
                if url.is_empty() {
                    self.state.notice = Some(crate::state::Notice {
                        message: "un MCP remoto necesita una URL".to_string(),
                        error: true,
                    });
                    return;
                }
                serde_json::json!({ "type": "remote", "url": url })
            }
        };
        self.daemon.send(Command::AddMcp {
            project: project.name.clone(),
            server: name,
            config,
        });
        self.mcp_form = McpForm::default();
    }

    pub fn open_settings(&mut self) {
        self.show_settings = true;
        self.settings_synced = false;
        self.sync_project_form();
        self.daemon.send(Command::LoadSettings);
    }

    /// Fills the global form once per opening, when the daemon answers.
    pub fn sync_global_form(&mut self) {
        if self.settings_synced {
            return;
        }
        if let Some(view) = &self.state.settings {
            self.global_form = GlobalForm::from_view(view);
            self.settings_synced = true;
        }
    }

    pub fn save_global_settings(&mut self) {
        match self.global_form.patch() {
            Ok(patch) => {
                self.settings_synced = false;
                self.daemon.send(Command::SaveSettings(patch));
            }
            Err(message) => {
                self.state.notice = Some(crate::state::Notice {
                    message,
                    error: true,
                });
            }
        }
    }

    pub fn save_project_form(&mut self) {
        let Some(id) = self.project_form.project.clone() else {
            return;
        };
        let budget_usd = parse_amount(&self.project_form.budget_usd);
        let budget_tokens = parse_tokens(&self.project_form.budget_tokens);
        if budget_usd.is_err() || budget_tokens.is_err() {
            self.state.notice = Some(crate::state::Notice {
                message: "presupuesto inválido: usa números positivos o déjalo vacío".to_string(),
                error: true,
            });
            return;
        }
        self.daemon.send(Command::UpdateProject {
            id,
            settings: jcs_proto::ProjectSettings {
                auto_merge: self.project_form.auto_merge,
                budget_usd_per_day: budget_usd.unwrap_or(None),
                budget_tokens_per_day: budget_tokens.unwrap_or(None),
            },
        });
    }

    pub fn queue_objective(&mut self) {
        let prompt = self.objective.trim().to_string();
        if prompt.is_empty() {
            return;
        }
        let Some(project) = self.state.project() else {
            self.state.notice = Some(crate::state::Notice {
                message: "añade un proyecto antes de encolar un objetivo".to_string(),
                error: true,
            });
            return;
        };
        let model = self.model.trim();
        self.daemon.send(Command::Ask {
            project: project.name.clone(),
            prompt,
            model: (!model.is_empty()).then(|| model.to_string()),
            plan: self.plan,
        });
        self.objective.clear();
    }
}

impl eframe::App for Studio {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        for update in self.daemon.poll() {
            self.state.apply(update);
        }
        self.shortcuts(context);
        self.sync_project();
        self.sync_task();
        self.terminals.poll();
        crate::ui::shell(context, self);
        crate::ui::windows(context, self);
        // Half a second is fast enough for a control plane and cheap when idle.
        context.request_repaint_after(Duration::from_millis(500));
    }
}

/// Parses an optional USD ceiling. `None` means "no ceiling", not zero.
pub fn parse_amount(value: &str) -> Result<Option<f64>, ()> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    match value.parse::<f64>() {
        Ok(amount) if amount > 0.0 && amount.is_finite() => Ok(Some(amount)),
        _ => Err(()),
    }
}

/// Parses an optional token ceiling.
pub fn parse_tokens(value: &str) -> Result<Option<i64>, ()> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    match value.parse::<i64>() {
        Ok(tokens) if tokens > 0 => Ok(Some(tokens)),
        _ => Err(()),
    }
}

fn format_amount(amount: f64) -> String {
    if amount.fract() == 0.0 {
        format!("{amount:.0}")
    } else {
        format!("{amount:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_global_form_round_trips_through_a_view() {
        let view = jcs_proto::SettingsView {
            default_model: Some("opencode-go/hy4-flash-free".to_string()),
            default_agent: None,
            planning_model: None,
            planning_agent: None,
            review_model: None,
            review: true,
            risk_max_lines: 250,
            run_timeout_secs: 600,
            context_tokens: 0,
            max_repair_cycles: 3,
            max_concurrent: 4,
            restart_required: Vec::new(),
            telegram_chat_ids: vec![42],
            agent_skills: Vec::new(),
            telegram_configured: true,
            telegram_token_hint: Some("abcd".to_string()),
        };
        let form = GlobalForm::from_view(&view);
        let patch = form.patch().unwrap();
        assert_eq!(
            patch.default_model.as_deref(),
            Some("opencode-go/hy4-flash-free")
        );
        assert_eq!(patch.review, Some(true));
        assert_eq!(patch.risk_max_lines, Some(250));
        assert_eq!(patch.context_tokens, Some(0));
        assert_eq!(patch.max_concurrent, Some(4));
        assert_eq!(patch.telegram_chat_ids, Some(vec![42]));
        assert!(patch.telegram_token.is_none());
    }

    #[test]
    fn the_global_form_rejects_a_non_numeric_limit() {
        let form = GlobalForm {
            run_timeout_secs: "pronto".to_string(),
            ..GlobalForm::default()
        };
        assert!(form.patch().is_err());
    }

    #[test]
    fn telegram_chat_ids_accept_separators_and_reject_junk() {
        assert_eq!(parse_chat_ids("1, 2  3"), Ok(vec![1, 2, 3]));
        assert_eq!(parse_chat_ids(""), Ok(Vec::new()));
        assert!(parse_chat_ids("1, cero").is_err());
        assert!(parse_chat_ids("0").is_err());
    }

    #[test]
    fn empty_budget_means_no_ceiling() {
        assert_eq!(parse_amount("  "), Ok(None));
        assert_eq!(parse_tokens(""), Ok(None));
    }

    #[test]
    fn budgets_must_be_positive_numbers() {
        assert_eq!(parse_amount("0"), Err(()));
        assert_eq!(parse_amount("-3"), Err(()));
        assert_eq!(parse_amount("abc"), Err(()));
        assert_eq!(parse_amount("12.5"), Ok(Some(12.5)));
        assert_eq!(parse_tokens("5000"), Ok(Some(5000)));
        assert_eq!(parse_tokens("0"), Err(()));
    }
}
