//! Telegram channel.
//!
//! Long polling, so the daemon never needs an inbound port. Every update is checked against
//! an explicit allowlist of chat ids: a bot token that leaks into the wrong group must not
//! become a remote control for the repository.
//!
//! Notifications read the event log, not the orchestrator: the channel observes facts, it
//! does not drive them.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use jcs_core::{EventKind, GoalId, GoalStatus, StatusMachine};
use serde_json::{json, Value};
use tokio::time::{interval, MissedTickBehavior};

use crate::state::AppState;

const TELEGRAM_API: &str = "https://api.telegram.org";

/// Starts the bot when a token and at least one chat are configured.
pub fn spawn(state: AppState) -> Option<tokio::task::JoinHandle<()>> {
    let token = state.secrets.telegram_token()?;
    let chats = state.runtime.telegram_chat_ids();
    if chats.is_empty() {
        tracing::warn!("JCS_TELEGRAM_TOKEN set but no chat ids; the bot stays off");
        return None;
    }

    Some(tokio::spawn(async move {
        tracing::info!(chats = chats.len(), "telegram bot started");

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(40))
            .build()
            .unwrap_or_default();

        let mut offset = 0i64;
        let mut cursor = jcs_store::latest_seq(&state.store).await.unwrap_or(0);
        let mut ticker = interval(Duration::from_secs(2));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            ticker.tick().await;

            if let Err(error) = poll_updates(&state, &client, &token, &chats, &mut offset).await {
                tracing::debug!(error = %format!("{error:#}"), "telegram polling failed");
            }

            if let Err(error) = notify_events(&state, &client, &token, &chats, &mut cursor).await {
                tracing::debug!(error = %format!("{error:#}"), "telegram notification failed");
            }
        }
    }))
}

async fn poll_updates(
    state: &AppState,
    client: &reqwest::Client,
    token: &str,
    chats: &[i64],
    offset: &mut i64,
) -> Result<()> {
    let response: Value = client
        .post(format!("{TELEGRAM_API}/bot{token}/getUpdates"))
        .json(&json!({ "offset": *offset, "timeout": 0 }))
        .send()
        .await?
        .json()
        .await?;

    let Some(updates) = response.get("result").and_then(Value::as_array) else {
        return Ok(());
    };

    for update in updates {
        if let Some(id) = update.get("update_id").and_then(Value::as_i64) {
            *offset = id + 1;
        }

        let Some(message) = update.get("message") else {
            continue;
        };
        let Some(chat_id) = message
            .get("chat")
            .and_then(|chat| chat.get("id"))
            .and_then(Value::as_i64)
        else {
            continue;
        };

        if !chats.contains(&chat_id) {
            tracing::warn!(
                chat_id,
                "ignoring a telegram message from an unauthorised chat"
            );
            continue;
        }

        let text = message.get("text").and_then(Value::as_str).unwrap_or("");
        let reply = handle_command(state, text).await;
        send_message(client, token, chat_id, &reply).await?;
    }

    Ok(())
}

/// Handles one command and returns the reply text. Public for tests.
pub(crate) async fn handle_command(state: &AppState, text: &str) -> String {
    match parse_command(text) {
        Command::Help => help_text(),
        Command::Status => status_text(state).await,
        Command::Projects => projects_text(state).await,
        Command::Goal(id) => goal_text(state, &id).await,
        Command::Resume(id) => resume_text(state, &id).await,
        Command::Ask(project, prompt) => ask_text(state, &project, &prompt).await,
        Command::Unknown(command) => format!("unknown command {command:?}. Try /help."),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    Status,
    Projects,
    Goal(String),
    Resume(String),
    Ask(String, String),
    Unknown(String),
}

fn parse_command(text: &str) -> Command {
    let text = text.trim();
    let Some(rest) = text.strip_prefix('/') else {
        return Command::Unknown(text.to_string());
    };

    let mut parts = rest.split_whitespace();
    let command = parts.next().unwrap_or_default();
    // `/status@my_bot` is how Telegram addresses a bot in a group.
    let command = command.split('@').next().unwrap_or(command);
    let argument = parts.next().unwrap_or_default().to_string();
    let remainder = parts.collect::<Vec<_>>().join(" ");

    match command {
        "help" | "start" => Command::Help,
        "status" => Command::Status,
        "projects" => Command::Projects,
        "goal" => Command::Goal(argument),
        "resume" => Command::Resume(argument),
        "ask" => Command::Ask(argument, remainder),
        other => Command::Unknown(format!("/{other}")),
    }
}

fn help_text() -> String {
    "/status — daemon health and totals\n\
     /projects — registered projects\n\
     /goal <id> — goal, tasks and runs\n\
     /resume <id> — reactivate a blocked goal\n\
     /ask <project> <objective> — queue an objective\n\
     /help — this message"
        .to_string()
}

async fn status_text(state: &AppState) -> String {
    let projects = state.store.list_projects().await.unwrap_or_default();
    let goals = state.store.list_goals(50).await.unwrap_or_default();
    let tasks = state.store.list_tasks(200).await.unwrap_or_default();

    let mut running = 0;
    let mut blocked = 0;
    let mut succeeded = 0;
    for goal in &goals {
        match goal.status {
            GoalStatus::Running => running += 1,
            GoalStatus::Blocked => blocked += 1,
            GoalStatus::Succeeded => succeeded += 1,
            _ => {}
        }
    }

    let cost = state.store.total_cost_usd(None).await.unwrap_or(0.0);

    format!(
        "jcsd up · {} projects · {} goals ({} running, {} blocked, {} succeeded)\n\
         {} tasks · ${cost:.4} spent",
        projects.len(),
        goals.len(),
        running,
        blocked,
        succeeded,
        tasks.len()
    )
}

async fn projects_text(state: &AppState) -> String {
    let projects = state.store.list_projects().await.unwrap_or_default();
    if projects.is_empty() {
        return "no projects; add one with `jcs project add <path>`".to_string();
    }

    projects
        .iter()
        .map(|project| {
            format!(
                "{} · {} · auto_merge={}",
                project.name, project.base_branch, project.auto_merge
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn goal_text(state: &AppState, id: &str) -> String {
    let Ok(goal_id) = GoalId::parse(id) else {
        return format!("invalid goal id {id:?}");
    };
    let Ok(Some(goal)) = state.store.get_goal(&goal_id).await else {
        return format!("no goal {id}");
    };

    let tasks = state
        .store
        .list_tasks_by_goal(&goal_id)
        .await
        .unwrap_or_default();
    let mut lines = vec![format!(
        "goal {} · {}\n{}",
        &goal.id.as_str()[5..13.min(goal.id.as_str().len())],
        goal.status.as_str(),
        truncate(&goal.prompt, 160)
    )];

    for task in tasks.iter().take(8) {
        lines.push(format!(
            "  {:<9} {}",
            task.status.as_str(),
            truncate(&task.title, 60)
        ));
    }
    if tasks.len() > 8 {
        lines.push(format!("  … {} more", tasks.len() - 8));
    }

    lines.join("\n")
}

async fn resume_text(state: &AppState, id: &str) -> String {
    let Ok(goal_id) = GoalId::parse(id) else {
        return format!("invalid goal id {id:?}");
    };
    let Ok(Some(goal)) = state.store.get_goal(&goal_id).await else {
        return format!("no goal {id}");
    };

    if goal.status != GoalStatus::Blocked {
        return format!("goal is {}, not blocked", goal.status.as_str());
    }

    if let Err(error) = state.store.clear_goal_merge_errors(&goal_id).await {
        return format!("cannot resume: {error}");
    }
    match state
        .store
        .set_goal_status(&goal_id, GoalStatus::Running)
        .await
    {
        Ok(_) => "goal resumed".to_string(),
        Err(error) => format!("cannot resume: {error}"),
    }
}

async fn ask_text(state: &AppState, project_reference: &str, prompt: &str) -> String {
    if project_reference.is_empty() || prompt.trim().is_empty() {
        return "usage: /ask <project> <objective>".to_string();
    }

    let Ok(Some(project)) = state.store.resolve_project(project_reference).await else {
        return format!("no project matches {project_reference:?}");
    };

    // A budget check here would duplicate the scheduler; the goal is queued and the
    // scheduler will park it if the day's budget is exhausted.
    let goal = match state.store.create_goal(&project.id, prompt).await {
        Ok(goal) => goal,
        Err(error) => return format!("cannot create the goal: {error}"),
    };

    crate::http::spawn_planning(state.clone(), goal.clone(), project);

    format!(
        "queued goal {}",
        &goal.id.as_str()[5..13.min(goal.id.as_str().len())]
    )
}

/// Sends terminal goal states and escalations to every allowed chat.
async fn notify_events(
    state: &AppState,
    client: &reqwest::Client,
    token: &str,
    chats: &[i64],
    cursor: &mut i64,
) -> Result<()> {
    let events = jcs_store::events_since(&state.store, *cursor, 200).await?;
    if events.is_empty() {
        return Ok(());
    }

    for event in &events {
        *cursor = event.seq;

        let message = match event.kind {
            EventKind::GoalStatusChanged => {
                let to = event
                    .payload
                    .get("to")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if !matches!(to, "succeeded" | "failed" | "blocked" | "cancelled") {
                    continue;
                }
                let summary = event
                    .payload
                    .get("prompt")
                    .and_then(Value::as_str)
                    .map(|prompt| truncate(prompt, 120))
                    .unwrap_or_default();
                format!("goal {to}\n{summary}")
            }
            EventKind::Error => {
                let category = event
                    .payload
                    .get("category")
                    .and_then(Value::as_str)
                    .unwrap_or("error");
                if !matches!(
                    category,
                    "needs_input"
                        | "budget_exceeded"
                        | "merge_failed"
                        | "promotion_failed"
                        | "planning_failed"
                ) {
                    continue;
                }
                let reason = event
                    .payload
                    .get("reason")
                    .or_else(|| event.payload.get("question"))
                    .and_then(Value::as_str)
                    .unwrap_or(category);
                format!("{category}\n{}", truncate(reason, 200))
            }
            _ => continue,
        };

        for chat in chats {
            if let Err(error) = send_message(client, token, *chat, &message).await {
                tracing::warn!(error = %format!("{error:#}"), "cannot send a telegram notification");
            }
        }
    }

    Ok(())
}

async fn send_message(
    client: &reqwest::Client,
    token: &str,
    chat_id: i64,
    text: &str,
) -> Result<()> {
    let response = client
        .post(format!("{TELEGRAM_API}/bot{token}/sendMessage"))
        .json(&json!({
            "chat_id": chat_id,
            "text": truncate(text, 3_800),
            "disable_web_page_preview": true,
        }))
        .send()
        .await
        .context("telegram request failed")?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow!(
            "telegram returned {status}: {}",
            truncate(&body, 200)
        ));
    }
    Ok(())
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    value.chars().take(max).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_parsed_with_and_without_arguments() {
        assert_eq!(parse_command("/status"), Command::Status);
        assert_eq!(parse_command("/status@jcs_bot"), Command::Status);
        assert_eq!(parse_command("/projects"), Command::Projects);
        assert_eq!(
            parse_command("/goal goal_abc"),
            Command::Goal("goal_abc".into())
        );
        assert_eq!(
            parse_command("/resume goal_abc"),
            Command::Resume("goal_abc".into())
        );
        assert_eq!(
            parse_command("/ask demo add validation"),
            Command::Ask("demo".into(), "add validation".into())
        );
        assert_eq!(parse_command("/help"), Command::Help);
        assert_eq!(parse_command("hello"), Command::Unknown("hello".into()));
        assert_eq!(parse_command("/nope"), Command::Unknown("/nope".into()));
    }

    #[test]
    fn ask_requires_both_parts() {
        assert_eq!(
            parse_command("/ask demo"),
            Command::Ask("demo".into(), String::new())
        );
        assert_eq!(
            parse_command("/ask"),
            Command::Ask(String::new(), String::new())
        );
    }

    #[test]
    fn long_messages_are_truncated() {
        let text = truncate(&"x".repeat(10_000), 100);
        assert!(text.chars().count() <= 101);
        assert!(text.ends_with('…'));
    }
}
