//! `jcs` — the JC Studio console client.
//!
//! A thin client: it renders, it asks, it never owns state. Everything durable lives in
//! `jcsd` behind the unix socket (ADR 0001).

mod client;
mod render;

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use jcs_core::EventKind;
use jcs_proto::{AddProjectRequest, AskRequest, PermissionMode};

use crate::client::Client;

#[derive(Debug, Parser)]
#[command(
    name = "jcs",
    version,
    about = "JC Studio: multi-agent console orchestrator",
    long_about = "Talks to jcsd over a unix socket. Run `jcsd` first, or `jcs doctor` to diagnose."
)]
struct Cli {
    /// Socket path (default: $XDG_RUNTIME_DIR/jcs/jcsd.sock).
    #[arg(long, env = "JCS_SOCKET", global = true)]
    socket: Option<PathBuf>,

    /// Machine-readable output.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Register and inspect projects.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Send an objective to an agent.
    Ask(AskArgs),
    /// Show daemon, project and task status.
    Status,
    /// Print or follow the event log.
    Logs(LogsArgs),
    /// Inspect runs and their gates.
    Runs(RunsArgs),
    /// Abort a running task.
    Abort {
        /// Run id, as printed by `jcs ask`.
        run: String,
    },
    /// Check that the daemon is answering, without touching the backend.
    Ping,
    /// Verify the local installation end to end.
    Doctor,
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    /// Register a git repository.
    Add {
        /// Path to the repository (resolved against your shell's directory).
        path: PathBuf,
        /// Short name; defaults to the directory name.
        #[arg(long)]
        name: Option<String>,
        /// Branch new task worktrees are cut from; defaults to the current branch.
        #[arg(long)]
        base_branch: Option<String>,
    },
    /// List registered projects.
    List,
    /// Remove a project by id or name (state is deleted; the audit log is kept).
    Remove {
        /// Project id or name.
        project: String,
    },
}

#[derive(Debug, Args)]
struct AskArgs {
    /// Project name or path. Defaults to the current directory.
    ///
    /// As a convenience, when `--project` is omitted and the first word names a
    /// registered project, it is used as the project and the rest is the objective.
    #[arg(long, short = 'p')]
    project: Option<String>,

    /// The objective, in words.
    #[arg(required = true, num_args = 1..)]
    prompt: Vec<String>,

    /// Model as provider/model, e.g. opencode/hy4-flash-free.
    #[arg(long)]
    model: Option<String>,

    /// Agent name, when the backend supports several.
    #[arg(long)]
    agent: Option<String>,

    /// Reject every permission request instead of auto-approving file work.
    #[arg(long)]
    strict: bool,

    /// Return as soon as the task is accepted, instead of following it.
    #[arg(long)]
    detach: bool,
}

#[derive(Debug, Args)]
struct LogsArgs {
    /// Only show events for this task id.
    #[arg(long)]
    task: Option<String>,

    /// Maximum number of events to print when not following.
    #[arg(long, default_value_t = 50)]
    limit: i64,

    /// Keep streaming new events.
    #[arg(long)]
    follow: bool,

    /// Start after this sequence number.
    #[arg(long, default_value_t = 0)]
    since: i64,
}

#[derive(Debug, Args)]
struct RunsArgs {
    /// Run id; omit to list recent runs.
    run: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let socket = cli
        .socket
        .clone()
        .unwrap_or_else(jcs_proto::paths::socket_path);
    let client = Client::new(socket);
    let json = cli.json;

    match cli.command {
        Command::Project { command } => project(&client, command, json).await,
        Command::Ask(args) => ask(&client, args, json).await,
        Command::Status => status(&client, json).await,
        Command::Logs(args) => logs(&client, args, json).await,
        Command::Runs(args) => runs(&client, args, json).await,
        Command::Abort { run } => {
            client.abort(&run).await?;
            println!("aborted {run}");
            Ok(())
        }
        Command::Ping => {
            client.health().await?;
            if json {
                println!("{{\"ok\":true}}");
            } else {
                println!("ok");
            }
            Ok(())
        }
        Command::Doctor => doctor(&client).await,
    }
}

async fn project(client: &Client, command: ProjectCommand, json: bool) -> Result<()> {
    match command {
        ProjectCommand::Add {
            path,
            name,
            base_branch,
        } => {
            // Resolve here: the daemon runs from a different working directory.
            let resolved = std::fs::canonicalize(&path)
                .with_context(|| format!("cannot resolve {}", path.display()))?;

            let request = AddProjectRequest {
                path: resolved.to_string_lossy().to_string(),
                name,
                base_branch,
            };
            let response = client.add_project(&request).await?;

            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                println!(
                    "added {} ({}) on branch {}",
                    response.project.name, response.project.id, response.project.base_branch
                );
            }
            Ok(())
        }
        ProjectCommand::List => {
            let response = client.projects().await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
                return Ok(());
            }
            if response.projects.is_empty() {
                println!("no projects; add one with `jcs project add <path>`");
                return Ok(());
            }
            for project in &response.projects {
                println!(
                    "{:<16} {:<12} {}",
                    project.name, project.base_branch, project.root_path
                );
            }
            Ok(())
        }
        ProjectCommand::Remove { project } => {
            let projects = client.projects().await?.projects;
            let found = projects
                .iter()
                .find(|candidate| candidate.id == project || candidate.name == project)
                .ok_or_else(|| {
                    anyhow::anyhow!("no project matches {project:?}; run `jcs project list`")
                })?;
            client.remove_project(&found.id).await?;
            println!("removed {} ({})", found.name, found.id);
            Ok(())
        }
    }
}

async fn ask(client: &Client, args: AskArgs, json: bool) -> Result<()> {
    let AskArgs {
        project,
        mut prompt,
        model,
        agent,
        strict,
        detach,
    } = args;

    let project = match project {
        Some(project) => project,
        None => {
            // "jcs ask demo fix the parser" works when `demo` is a registered project.
            let first_is_project = prompt.len() >= 2
                && client
                    .projects()
                    .await
                    .map(|response| {
                        response.projects.iter().any(|candidate| {
                            candidate.name == prompt[0] || candidate.root_path == prompt[0]
                        })
                    })
                    .unwrap_or(false);

            if first_is_project {
                prompt.remove(0)
            } else {
                std::env::current_dir()
                    .context("cannot determine the current directory")?
                    .to_string_lossy()
                    .to_string()
            }
        }
    };

    let prompt = prompt.join(" ");
    if prompt.trim().is_empty() {
        bail!("the objective is empty");
    }

    // Capture the cursor *before* asking, so no event of this run can be missed.
    let cursor = client.latest_seq().await?;

    let request = AskRequest {
        project,
        prompt,
        model,
        agent,
        permissions: if strict {
            PermissionMode::Strict
        } else {
            PermissionMode::Auto
        },
        title: None,
    };
    let response = client.ask(&request).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
        if detach {
            return Ok(());
        }
    } else {
        println!(
            "goal {} · task {} · run {}",
            response.goal_id, response.task_id, response.run_id
        );
        if detach {
            println!("detached; follow with `jcs logs --follow`");
            return Ok(());
        }
    }

    follow_run(client, &response.run_id, cursor).await
}

async fn follow_run(client: &Client, run_id: &str, cursor: i64) -> Result<()> {
    let mut stream = client.events(cursor).await?;

    while let Some(item) = stream.next().await {
        let event = item?;
        if event.run_id.as_ref().map(|id| id.as_str()) != Some(run_id) {
            continue;
        }

        let finished = event.kind == EventKind::RunFinished;
        render::event(&event, true);

        if finished {
            let detail = client.run_detail(run_id).await?;
            println!();
            render::run_detail(&detail);
            return match detail.run.status.as_str() {
                "succeeded" => Ok(()),
                other => bail!("run ended as {other}"),
            };
        }
    }

    bail!("the event stream closed before the run finished")
}

async fn status(client: &Client, json: bool) -> Result<()> {
    let response = client.status().await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        render::status(&response);
    }
    Ok(())
}

async fn logs(client: &Client, args: LogsArgs, json: bool) -> Result<()> {
    if args.follow {
        let mut stream = client.events(args.since).await?;
        while let Some(item) = stream.next().await {
            let event = item?;
            if let Some(task) = &args.task {
                if event.task_id.as_ref().map(|id| id.as_str()) != Some(task.as_str()) {
                    continue;
                }
            }
            if json {
                println!("{}", serde_json::to_string(&event)?);
            } else {
                render::event(&event, true);
            }
        }
        return Ok(());
    }

    let events = client
        .event_history(
            if args.since > 0 {
                Some(args.since)
            } else {
                None
            },
            args.limit,
        )
        .await?;

    for event in events {
        if let Some(task) = &args.task {
            if event.task_id.as_ref().map(|id| id.as_str()) != Some(task.as_str()) {
                continue;
            }
        }
        if json {
            println!("{}", serde_json::to_string(&event)?);
        } else {
            render::event(&event, true);
        }
    }
    Ok(())
}

async fn runs(client: &Client, args: RunsArgs, json: bool) -> Result<()> {
    match args.run {
        Some(id) => {
            let detail = client.run_detail(&id).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&detail)?);
            } else {
                render::run_detail(&detail);
            }
        }
        None => {
            let runs = client.runs().await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&runs)?);
                return Ok(());
            }
            if runs.is_empty() {
                println!("no runs yet");
                return Ok(());
            }
            for run in &runs {
                println!(
                    "{:<10} {:<10} attempt {} · ${:.4}{}",
                    run.status,
                    render::short_id(&run.id),
                    run.attempt,
                    run.cost_usd,
                    run.error
                        .as_deref()
                        .map(|error| format!(" · {error}"))
                        .unwrap_or_default()
                );
            }
        }
    }
    Ok(())
}

async fn doctor(client: &Client) -> Result<()> {
    let mut failures = 0usize;

    let socket = client.socket();
    match std::fs::metadata(socket) {
        Ok(metadata) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = metadata.permissions().mode() & 0o777;
                let ok = mode & 0o077 == 0;
                if !ok {
                    failures += 1;
                }
                render::doctor_check(
                    "socket permissions",
                    ok,
                    &format!("{} (mode {mode:o})", socket.display()),
                );
            }
            #[cfg(not(unix))]
            render::doctor_check("socket", true, &socket.display().to_string());
        }
        Err(error) => {
            failures += 1;
            render::doctor_check("socket", false, &format!("{}: {error}", socket.display()));
        }
    }

    match client.health().await {
        Ok(health) => {
            render::doctor_check(
                "daemon",
                true,
                &format!(
                    "v{} · protocol {} · up {}s · db {}",
                    health.version, health.protocol, health.uptime_secs, health.database
                ),
            );
            if health.protocol != jcs_proto::PROTOCOL_VERSION {
                failures += 1;
                render::doctor_check(
                    "protocol",
                    false,
                    &format!(
                        "client {} vs daemon {}",
                        jcs_proto::PROTOCOL_VERSION,
                        health.protocol
                    ),
                );
            }

            let reachable = health.agent.reachable;
            if !reachable {
                failures += 1;
            }
            render::doctor_check(
                "agent backend",
                reachable,
                &match (&health.agent.version, &health.agent.message) {
                    (Some(version), _) => format!("opencode {version}"),
                    (None, Some(message)) => message.clone(),
                    _ => "unreachable".to_string(),
                },
            );
        }
        Err(error) => {
            failures += 1;
            render::doctor_check("daemon", false, &format!("{error:#}"));
        }
    }

    if failures > 0 {
        bail!("{failures} check(s) failed");
    }
    println!("\nall checks passed");
    Ok(())
}
