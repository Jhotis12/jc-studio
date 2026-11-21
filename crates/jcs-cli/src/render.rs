//! Terminal rendering. Plain text on purpose: it stays readable when piped, logged or
//! pasted, and colour adds a dependency without adding information.

use jcs_core::{Event, EventKind, GateStatus, StatusMachine};

/// Renders one event. Assistant text is printed inline so a run reads like a transcript.
pub fn event(event: &Event, scope_is_current: bool) {
    let ts = event.ts.format("%H:%M:%S");

    match event.kind {
        EventKind::AgentText => {
            if scope_is_current {
                if let Some(text) = event.payload.get("text").and_then(|value| value.as_str()) {
                    print!("{text}");
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                }
            }
        }
        EventKind::AgentToolCalled => {
            let tool = event
                .payload
                .get("tool")
                .and_then(|value| value.as_str())
                .unwrap_or("?");
            println!("[{ts}] -> {tool}");
        }
        EventKind::AgentToolFinished => {
            let payload = &event.payload;
            let kind = payload.get("kind").and_then(|value| value.as_str());
            if kind == Some("permission_decision") {
                let action = payload
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let decision = payload
                    .get("decision")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                println!("[{ts}] permission {action}: {decision}");
            } else {
                let tool = payload
                    .get("tool")
                    .and_then(|value| value.as_str())
                    .unwrap_or("?");
                let failed = payload
                    .get("failed")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
                println!("[{ts}] <- {tool}{}", if failed { " (failed)" } else { "" });
            }
        }
        EventKind::GateFinished => {
            let gate = event
                .payload
                .get("gate")
                .and_then(|value| value.as_str())
                .unwrap_or("gate");
            let status = event
                .payload
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("?");
            println!("[{ts}] gate {gate}: {status}");
        }
        EventKind::CommitCreated => {
            let short = event
                .payload
                .get("short")
                .and_then(|value| value.as_str())
                .unwrap_or("?");
            let message = event
                .payload
                .get("message")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            println!("[{ts}] commit {short} {message}");
        }
        EventKind::RunFinished => {
            let status = event
                .payload
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("?");
            println!("\n[{ts}] run {status}");
        }
        EventKind::TaskStatusChanged => {
            let to = event
                .payload
                .get("to")
                .and_then(|value| value.as_str())
                .unwrap_or("?");
            println!("[{ts}] task -> {to}");
        }
        EventKind::Error => {
            let reason = event
                .payload
                .get("reason")
                .and_then(|value| value.as_str())
                .unwrap_or_else(|| {
                    event
                        .payload
                        .get("message")
                        .and_then(|value| value.as_str())
                        .unwrap_or("unspecified error")
                });
            println!("[{ts}] ERROR {reason}");
        }
        _ => {}
    }
}

pub fn status(response: &jcs_proto::StatusResponse) {
    let health = &response.health;
    println!(
        "JC Studio {} · protocol {} · up {} · backend {} {}",
        health.version,
        health.protocol,
        duration(health.uptime_secs),
        health.agent.backend,
        match (&health.agent.version, health.agent.reachable) {
            (Some(version), true) => format!("{version} (reachable)"),
            (_, true) => "reachable".to_string(),
            (_, false) => "UNREACHABLE".to_string(),
        }
    );
    if let Some(message) = &health.agent.message {
        println!("  backend: {message}");
    }
    println!();

    if response.projects.is_empty() {
        println!("No projects. Add one with: jcs project add <path>");
    } else {
        println!("PROJECTS");
        for project in &response.projects {
            println!(
                "  {:<16} {:<12} {}",
                project.name, project.base_branch, project.root_path
            );
        }
    }

    if !response.tasks.is_empty() {
        println!("\nTASKS");
        for task in &response.tasks {
            println!(
                "  {:<10} {:<12} {}",
                task.status,
                short_id(&task.id),
                truncate(&task.title, 60)
            );
        }
    }

    if !response.runs.is_empty() {
        println!("\nRUNS");
        for run in &response.runs {
            println!(
                "  {:<10} {:<10} attempt {} · ${:.4} · {} in / {} out{}",
                run.status,
                short_id(&run.id),
                run.attempt,
                run.cost_usd,
                run.tokens_in,
                run.tokens_out,
                run.error
                    .as_deref()
                    .map(|error| format!(" · {error}"))
                    .unwrap_or_default()
            );
        }
    }

    println!(
        "\nTOTAL ${:.4} · {} tokens in / {} out · {} · {}",
        response.totals.cost_usd,
        response.totals.tokens_in,
        response.totals.tokens_out,
        plural(response.totals.tasks, "task", "tasks"),
        plural(response.totals.projects, "project", "projects"),
    );
}

pub fn run_detail(detail: &jcs_proto::RunDetailResponse) {
    let run = &detail.run;
    println!(
        "run {} · {} · attempt {} · ${:.4}",
        short_id(&run.id),
        run.status,
        run.attempt,
        run.cost_usd
    );

    if let Some(task) = &detail.task {
        println!("task {} · {}", short_id(&task.id), task.title);
        if let Some(branch) = &task.branch {
            println!("branch {branch}");
        }
        if let Some(worktree) = &task.worktree {
            println!("worktree {worktree}");
        }
    }

    if !detail.gates.is_empty() {
        println!("\ngates");
        for gate in &detail.gates {
            let marker = if gate.status == GateStatus::Passed.as_str() {
                "ok"
            } else {
                "FAIL"
            };
            println!(
                "  [{marker}] {:<20} {} ms{}",
                gate.name,
                gate.duration_ms,
                gate.exit_code
                    .map(|code| format!(" · exit {code}"))
                    .unwrap_or_default()
            );
        }
    }

    if let Some(error) = &run.error {
        println!("\nerror: {error}");
    }
}

pub fn doctor_check(name: &str, ok: bool, detail: &str) {
    println!("[{}] {name:<24} {detail}", if ok { "ok" } else { "FAIL" });
}

pub fn short_id(id: &str) -> &str {
    let body = id.split_once('_').map(|(_, rest)| rest).unwrap_or(id);
    &body[..body.len().min(8)]
}

fn plural(count: i64, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    value.chars().take(max).collect::<String>() + "…"
}

fn duration(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3_600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h{}m", secs / 3_600, (secs % 3_600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_ids_drop_the_prefix_and_bound_the_length() {
        assert_eq!(short_id("task_9f2c8ab1deadbeef"), "9f2c8ab1");
        assert_eq!(short_id("noprefix"), "noprefix");
    }

    #[test]
    fn plurals_read_naturally() {
        assert_eq!(plural(1, "task", "tasks"), "1 task");
        assert_eq!(plural(0, "task", "tasks"), "0 tasks");
        assert_eq!(plural(3, "task", "tasks"), "3 tasks");
    }

    #[test]
    fn durations_are_human_scaled() {
        assert_eq!(duration(5), "5s");
        assert_eq!(duration(90), "1m");
        assert_eq!(duration(3_700), "1h1m");
    }
}
