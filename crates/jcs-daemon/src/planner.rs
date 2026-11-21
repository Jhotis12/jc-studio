//! Goal decomposition.
//!
//! The planner is a language model, so its output is untrusted: we extract a JSON object,
//! parse it, validate it against the domain rules, and ask once more with the concrete
//! error if it is wrong. Prose is never parsed.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use jcs_agent::{ModelRef, OpenCodeClient};
use jcs_core::{Goal, Plan, Project, TaskRole};

use crate::state::AppState;

/// How long the planner gets before the goal is failed with a clear reason.
const PLANNER_TIMEOUT: Duration = Duration::from_secs(180);
/// Attempts: the first answer plus one repair.
const MAX_ATTEMPTS: usize = 2;

/// Plans a goal and returns a validated plan, or an error that says what was wrong.
pub async fn plan_goal(state: &AppState, goal: &Goal, project: &Project) -> Result<Plan> {
    let agent = state
        .agent
        .get()
        .await
        .map_err(|error| anyhow!(error.message))?;

    // The model named in the request belongs to the goal, not only to its tasks. Planning
    // with a different (often unavailable) default is how `--model` silently did nothing.
    let goal_settings = state
        .store
        .goal_settings(&goal.id)
        .await
        .context("cannot read the goal settings for planning")?;
    let model = state
        .planning_model()
        .or(goal_settings.model)
        .or_else(|| state.default_model())
        .map(|reference| ModelRef::parse(&reference))
        .transpose()?;
    let planning_agent = state.planning_agent().or(goal_settings.agent);
    let session = agent
        .create_session(jcs_agent::CreateSession {
            title: "plan",
            directory: &project.root_path,
            model,
            agent: planning_agent.as_deref(),
            permissions: None,
        })
        .await
        .context("cannot open a planning session")?;

    let result = plan_in_session(&agent, &session.id, goal, project).await;

    // The planning session has no further use; stop it so it cannot accumulate cost.
    if let Err(error) = agent.interrupt(&session.id).await {
        tracing::debug!(error = %error, "cannot interrupt the planning session");
    }

    result
}

async fn plan_in_session(
    agent: &OpenCodeClient,
    session_id: &str,
    goal: &Goal,
    project: &Project,
) -> Result<Plan> {
    let mut prompt = planning_prompt(goal, project);

    for attempt in 1..=MAX_ATTEMPTS {
        agent
            .prompt(session_id, &prompt, "queue")
            .await
            .context("cannot deliver the planning prompt")?;

        let response = collect_response(agent, session_id).await?;

        let Some(json) = extract_json_object(&response) else {
            prompt = format!(
                "Your previous answer contained no JSON object. {}\n\
                 Return ONLY the JSON object, with no prose and no markdown fences.",
                format_hint()
            );
            tracing::warn!(attempt, "planner returned no JSON object");
            continue;
        };

        match serde_json::from_str::<Plan>(json) {
            Ok(plan) if plan.tasks.is_empty() => {
                return Err(anyhow!(
                    "the message is not a concrete objective; describe the change you want"
                ));
            }
            Ok(plan) => match plan.validate() {
                Ok(()) => return Ok(plan),
                Err(error) => {
                    prompt = format!(
                        "Your previous plan is invalid: {error}.\n\
                         Return ONLY the corrected JSON object, no prose.",
                    );
                    tracing::warn!(attempt, error = %error, "planner produced an invalid plan");
                }
            },
            Err(error) => {
                prompt = format!(
                    "Your previous answer was not valid JSON for the plan schema: {error}.\n\
                     {}\nReturn ONLY the corrected JSON object, no prose.",
                    format_hint()
                );
                tracing::warn!(attempt, error = %error, "planner produced invalid JSON");
            }
        }
    }

    Err(anyhow!(
        "the planner did not produce a valid plan after {MAX_ATTEMPTS} attempts"
    ))
}

/// Reads the assistant text of the planning turn.
///
/// Deliberately **not** built on the event stream: a fast reply can finish before the
/// subscription is established, and a missed terminal event would turn a good answer into a
/// timeout. The authoritative source is the session state (idle or not) plus the stored
/// messages, which are already durable on the backend.
pub(crate) async fn collect_response(agent: &OpenCodeClient, session_id: &str) -> Result<String> {
    let deadline = std::time::Instant::now() + PLANNER_TIMEOUT;

    loop {
        let session = agent
            .session(session_id)
            .await
            .context("cannot read the planning session state")?;

        if session.time.is_idle() {
            match session.outcome.as_deref() {
                Some("succeeded") | None => break,
                Some("failed") => return Err(anyhow!("the planner reported a failed execution")),
                Some("interrupted") => {
                    return Err(anyhow!("the planning execution was interrupted"))
                }
                Some(other) => {
                    return Err(anyhow!("the planner reported an unknown outcome {other:?}"))
                }
            }
        }

        // A planner that asks a question would never become idle; cancel it and let the
        // turn finish with whatever it produced.
        cancel_forms(agent, session_id).await;

        if std::time::Instant::now() >= deadline {
            return Err(anyhow!(
                "the planner did not answer within {}s",
                PLANNER_TIMEOUT.as_secs()
            ));
        }

        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    }

    let messages = agent
        .messages(session_id, 10)
        .await
        .context("cannot read the planning transcript")?;

    Ok(assistant_text(&messages))
}

/// Concatenates the text blocks of the newest assistant message.
fn assistant_text(messages: &[serde_json::Value]) -> String {
    for message in messages {
        if message.get("type").and_then(|value| value.as_str()) != Some("assistant") {
            continue;
        }
        let Some(content) = message.get("content").and_then(|value| value.as_array()) else {
            continue;
        };
        let text = content
            .iter()
            .filter(|block| block.get("type").and_then(|value| value.as_str()) == Some("text"))
            .filter_map(|block| block.get("text").and_then(|value| value.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        if !text.trim().is_empty() {
            return text;
        }
    }
    String::new()
}

async fn cancel_forms(agent: &OpenCodeClient, session_id: &str) {
    if let Ok(forms) = agent.list_forms(session_id).await {
        for form in forms {
            if let Some(id) = form.get("id").and_then(|value| value.as_str()) {
                let _ = agent.cancel_form(session_id, id).await;
            }
        }
    }
}

fn planning_prompt(goal: &Goal, project: &Project) -> String {
    format!(
        "You are planning engineering work. Do not write code. You may read files in the \
         repository to ground the plan.\n\
         \n\
         Objective:\n{prompt}\n\
         \n\
         Project: {name}\n\
         Root: {root}\n\
         Base branch: {base}\n\
         \n\
         Return ONLY a JSON object, no prose and no markdown fences:\n\
         {schema}\n\
         \n\
         Rules:\n\
         - Between 1 and 8 tasks. Each task must be independently verifiable.\n\
         - `key` is unique, lowercase letters, digits and dashes.\n\
         - `depends_on` references keys in this plan and must not form a cycle.\n\
         - `paths` are repository-relative prefixes the task expects to touch. Tasks that \
           can run in parallel must not claim overlapping paths.\n\
         - Prefer parallel work. Independent tasks use an empty `depends_on` and \
           non-overlapping paths; do not chain tasks just because they are listed in order. \
           Only a final synthesis or integration task should depend on all the others.\n\
         - `role` is one of: implement, test, review, docs, migrate.\n\
         - `description` must stand alone: it becomes the implementer's prompt.\n",
        prompt = goal.prompt,
        name = project.name,
        root = project.root_path.display(),
        base = project.base_branch,
        schema = format_hint(),
    )
}

fn format_hint() -> String {
    format!(
        "{{\n\
         \"tasks\": [\n\
         {{\n\
         \"key\": \"add-validation\",\n\
         \"title\": \"add email validation\",\n\
         \"description\": \"Implement validation of the email field in the registration endpoint, including tests.\",\n\
         \"depends_on\": [],\n\
         \"paths\": [\"src/registration\"],\n\
         \"role\": \"{}\"\n\
         }}\n\
         ]\n\
         }}",
        TaskRole::Implement.as_str()
    )
}

/// Extracts the first balanced JSON object from model output.
///
/// Handles the three shapes models actually produce: bare JSON, JSON inside a ```json fence,
/// and JSON surrounded by prose.
pub fn extract_json_object(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let start = text.find('{')?;

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for index in start..bytes.len() {
        let byte = bytes[index];

        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match byte {
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(&text[start..=index]);
                }
            }
            _ => {}
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_bare_json() {
        let text = r#"{"tasks":[]}"#;
        assert_eq!(extract_json_object(text), Some(text));
    }

    #[test]
    fn extracts_json_from_a_markdown_fence() {
        let text = "Here is the plan:\n```json\n{\"tasks\": [{\"key\": \"a\"}]}\n```\nDone.";
        assert_eq!(
            extract_json_object(text),
            Some("{\"tasks\": [{\"key\": \"a\"}]}")
        );
    }

    #[test]
    fn extracts_json_surrounded_by_prose() {
        let text = "Sure. {\"tasks\": []} Let me know.";
        assert_eq!(extract_json_object(text), Some("{\"tasks\": []}"));
    }

    #[test]
    fn braces_inside_strings_do_not_close_the_object() {
        let text = r#"{"title": "use { and } carefully", "tasks": []}"#;
        assert_eq!(extract_json_object(text), Some(text));
    }

    #[test]
    fn escaped_quotes_do_not_confuse_the_scanner() {
        let text = r#"{"title": "a \"quoted\" {value}", "tasks": []}"#;
        assert_eq!(extract_json_object(text), Some(text));
    }

    #[test]
    fn assistant_text_reads_the_newest_message_and_ignores_reasoning() {
        let messages = vec![
            serde_json::json!({
                "type": "assistant",
                "content": [{"type": "text", "text": "{\"tasks\":[]}"}]
            }),
            serde_json::json!({
                "type": "assistant",
                "content": [{"type": "text", "text": "older answer"}]
            }),
            serde_json::json!({"type": "user", "text": "objective"}),
        ];
        assert_eq!(assistant_text(&messages), "{\"tasks\":[]}");

        let reasoning_only = vec![serde_json::json!({
            "type": "assistant",
            "content": [{"type": "reasoning", "text": "thinking"}]
        })];
        assert!(assistant_text(&reasoning_only).is_empty());
    }

    #[test]
    fn prose_without_json_returns_none() {
        assert!(extract_json_object("I cannot plan this.").is_none());
        assert!(extract_json_object("{\"unterminated\": true").is_none());
    }

    #[test]
    fn the_prompt_forbids_prose_and_names_the_rules() {
        let goal = Goal {
            id: jcs_core::GoalId::new(),
            project_id: jcs_core::ProjectId::new(),
            prompt: "add a feature".into(),
            status: jcs_core::GoalStatus::Pending,
            created_at: jiff::Timestamp::now(),
            updated_at: jiff::Timestamp::now(),
        };
        let project = Project {
            id: jcs_core::ProjectId::new(),
            name: "demo".into(),
            root_path: std::path::PathBuf::from("/tmp/demo"),
            base_branch: "main".into(),
            created_at: jiff::Timestamp::now(),
            auto_merge: false,
            budget_usd_per_day: None,
            budget_tokens_per_day: None,
        };

        let prompt = planning_prompt(&goal, &project);
        assert!(prompt.contains("Return ONLY a JSON object"));
        assert!(prompt.contains("add a feature"));
        assert!(prompt.contains("must not form a cycle"));
    }
}
