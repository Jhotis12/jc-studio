//! Streaming turn for the project-level orchestrator.
//!
//! The HTTP request only starts the turn; the answer is written to the event log in chunks
//! so the app renders it as it arrives. The final assistant message is persisted in
//! `orchestrator_messages`.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use jcs_core::{EventKind, NewEvent, Project};
use tokio::sync::OwnedMutexGuard;
use tokio::time::{interval, MissedTickBehavior};

use crate::state::AppState;

const FLUSH_INTERVAL: Duration = Duration::from_millis(250);
const POLL_INTERVAL: Duration = Duration::from_millis(700);
const TURN_TIMEOUT: Duration = Duration::from_secs(180);

/// Starts a turn in the background. The guard is held until the answer finishes, so two
/// messages cannot interleave in one session.
pub fn spawn_turn(state: AppState, project: Project, prompt: String, guard: OwnedMutexGuard<()>) {
    tokio::spawn(async move {
        if let Err(error) = run(&state, &project, &prompt).await {
            let message = format!("{error:#}");
            tracing::error!(error = %message, "orchestrator turn failed");
            if let Err(error) = finish(&state, &project, "", Some(&message)).await {
                tracing::warn!(error = %error, "cannot record the orchestrator failure");
            }
        }
        drop(guard);
    });
}

async fn run(state: &AppState, project: &Project, prompt: &str) -> Result<()> {
    let agent = state
        .agent
        .get()
        .await
        .map_err(|error| anyhow!(error.message))?;

    let mut session_id = state.store.orchestrator_session(&project.id).await?;
    if let Some(id) = &session_id {
        if agent.session(id).await.is_err() {
            // OpenCode restarted or archived the session; start a fresh one.
            session_id = None;
        }
    }
    let mut fresh = false;
    let session_id = match session_id {
        Some(id) => id,
        None => {
            let model = state
                .planning_model()
                .or_else(|| state.default_model())
                .map(|reference| jcs_agent::ModelRef::parse(&reference))
                .transpose()?;
            let agent_name = state.planning_agent();
            let session = agent
                .create_session(jcs_agent::CreateSession {
                    title: "orchestrator",
                    directory: &project.root_path,
                    model,
                    agent: agent_name.as_deref(),
                    permissions: None,
                })
                .await
                .context("cannot open the orchestrator session")?;
            state
                .store
                .set_orchestrator_session(&project.id, &session.id)
                .await?;
            fresh = true;
            session.id
        }
    };

    let message = if fresh {
        format!(
            "You are the JC Studio orchestrator for the project {name} at {root}. \
             Talk with the operator, inspect the project when it helps, and answer their \
             questions. Do not modify files from this chat. When they want work executed, \
             propose a concrete, self-contained objective that they can approve with one \
             click; never claim to have created or executed it yourself.\n\nOperator: {prompt}",
            name = project.name,
            root = project.root_path.display(),
        )
    } else {
        prompt.to_string()
    };

    // Subscribe before prompting: a fast answer can finish before a later subscription.
    let mut stream = agent
        .events()
        .await
        .context("cannot subscribe to the orchestrator event stream")?;
    agent
        .prompt(&session_id, &message, "queue")
        .await
        .context("cannot deliver the message to the orchestrator")?;

    let mut full = String::new();
    let mut buffer = String::new();
    let mut reasoning_buffer = String::new();
    let mut flush = interval(FLUSH_INTERVAL);
    flush.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut poll = interval(POLL_INTERVAL);
    poll.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let deadline = tokio::time::sleep(TURN_TIMEOUT);
    tokio::pin!(deadline);
    let mut started = false;

    loop {
        tokio::select! {
            _ = &mut deadline => {
                flush_chunk(state, project, &session_id, &mut buffer).await?;
                flush_reasoning(state, project, &session_id, &mut reasoning_buffer).await?;
                return Err(anyhow!("the orchestrator did not answer within {}s", TURN_TIMEOUT.as_secs()));
            }
            event = stream.next_event() => {
                let Some(event) = event else {
                    return Err(anyhow!("the orchestrator event stream ended"));
                };
                let event = event?;
                if event.session_id() != Some(session_id.as_str()) {
                    continue;
                }
                if let Some(delta) = event.text_delta() {
                    buffer.push_str(delta);
                    full.push_str(delta);
                }
                if let Some(delta) = event.reasoning_delta() {
                    reasoning_buffer.push_str(delta);
                }
                if let Some(outcome) = event.execution_outcome() {
                    match outcome {
                        jcs_agent::ExecutionOutcome::Succeeded => break,
                        jcs_agent::ExecutionOutcome::Failed => {
                            return Err(anyhow!("the orchestrator reported a failed execution"));
                        }
                        jcs_agent::ExecutionOutcome::Interrupted => {
                            return Err(anyhow!("the orchestrator execution was interrupted"));
                        }
                    }
                }
            }
            _ = flush.tick() => {
                flush_chunk(state, project, &session_id, &mut buffer).await?;
                flush_reasoning(state, project, &session_id, &mut reasoning_buffer).await?;
            }
            _ = poll.tick() => {
                let session = agent.session(&session_id).await?;
                if !session.time.is_idle() {
                    started = true;
                } else if started {
                    break;
                }
                if started && session.outcome.as_deref() == Some("failed") {
                    return Err(anyhow!("the orchestrator reported a failed execution"));
                }
            }
        }
    }

    flush_chunk(state, project, &session_id, &mut buffer).await?;
    flush_reasoning(state, project, &session_id, &mut reasoning_buffer).await?;
    let final_text = if full.trim().is_empty() {
        let messages = agent
            .messages(&session_id, 10)
            .await
            .context("cannot read the orchestrator transcript")?;
        crate::planner::assistant_text(&messages)
    } else {
        full
    };

    finish(state, project, final_text.trim(), None).await
}

async fn flush_chunk(
    state: &AppState,
    project: &Project,
    session_id: &str,
    buffer: &mut String,
) -> Result<()> {
    if buffer.trim().is_empty() {
        buffer.clear();
        return Ok(());
    }
    let chunk = std::mem::take(buffer);
    let mut event = NewEvent::new(EventKind::OrchestratorText).project(&project.id);
    event.payload = serde_json::json!({
        "session_id": session_id,
        "text": chunk,
    });
    jcs_store::append_event(state.store.as_ref(), event).await?;
    Ok(())
}

async fn flush_reasoning(
    state: &AppState,
    project: &Project,
    session_id: &str,
    buffer: &mut String,
) -> Result<()> {
    if buffer.trim().is_empty() {
        buffer.clear();
        return Ok(());
    }
    let chunk = std::mem::take(buffer);
    let mut event = NewEvent::new(EventKind::OrchestratorReasoning).project(&project.id);
    event.payload = serde_json::json!({
        "session_id": session_id,
        "text": chunk,
    });
    jcs_store::append_event(state.store.as_ref(), event).await?;
    Ok(())
}

async fn finish(
    state: &AppState,
    project: &Project,
    text: &str,
    error: Option<&str>,
) -> Result<()> {
    if !text.trim().is_empty() {
        state
            .store
            .append_orchestrator_message(&project.id, "assistant", text)
            .await?;
    }
    let mut event = NewEvent::new(EventKind::OrchestratorFinished).project(&project.id);
    event.payload = serde_json::json!({
        "text": text,
        "error": error,
    });
    jcs_store::append_event(state.store.as_ref(), event).await?;
    Ok(())
}
