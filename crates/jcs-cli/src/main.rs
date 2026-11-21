//! `jcs` — the JC Studio console client.
//!
//! A thin client: it renders, it asks, it never owns state. Everything durable lives in
//! `jcsd` behind the unix socket (ADR 0001).

mod client;
mod render;
mod tui;

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
    /// Build or refresh the code index.
    Index {
        /// Project name or path; defaults to the current directory.
        project: Option<String>,
        /// Re-parse every file, ignoring content hashes.
        #[arg(long)]
        force: bool,
    },
    /// Search the code index.
    Search {
        /// Project name or path; defaults to the current directory.
        #[arg(long, short = 'p')]
        project: Option<String>,
        /// What to search for.
        #[arg(required = true, num_args = 1..)]
        query: Vec<String>,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Print daemon metrics in Prometheus text format.
    Metrics,
    /// Show index statistics.
    Memory {
        /// Project name or path; defaults to the current directory.
        project: Option<String>,
    },
    /// Show a goal with its tasks and runs.
    Goal {
        /// Goal id, as printed by `jcs ask`.
        goal: String,
    },
    /// Resume a blocked goal (for example after a budget reset).
    Resume {
        /// Goal id or a unique prefix.
        goal: String,
    },
    /// Inspect and render the project policy.
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
    /// Inspect and run verification gates.
    Gates {
        #[command(subcommand)]
        command: GatesCommand,
    },
    /// Abort a running task.
    Abort {
        /// Run id, as printed by `jcs ask`.
        run: String,
    },
    /// Check that the daemon is answering, without touching the backend.
    Ping,
    /// Verify the local installation end to end.
    Doctor,
    /// Open the terminal interface.
    Tui,
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
    /// Change auto-merge and daily budgets.
    Settings {
        /// Project id or name.
        project: String,
        /// Merge succeeded tasks into the base branch automatically.
        #[arg(long)]
        auto_merge: Option<bool>,
        /// Daily spend ceiling in US dollars.
        #[arg(long)]
        budget_usd: Option<f64>,
        /// Daily token ceiling.
        #[arg(long)]
        budget_tokens: Option<i64>,
        /// Remove both budget ceilings.
        #[arg(long)]
        clear_budgets: bool,
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

    /// Return as soon as the goal is accepted, instead of following it.
    #[arg(long)]
    detach: bool,

    /// Execute the objective as a single task instead of planning a DAG.
    #[arg(long)]
    no_plan: bool,
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

#[derive(Debug, Subcommand)]
enum PolicyCommand {
    /// List the packs available to the daemon.
    Packs,
    /// Show the effective policy of a project.
    Show {
        /// Project name or path; defaults to the current directory.
        project: Option<String>,
    },
    /// Render the derived files, or verify them with --check.
    Render {
        /// Project name or path; defaults to the current directory.
        project: Option<String>,
        /// Fail if the files on disk differ from the policy; write nothing.
        #[arg(long)]
        check: bool,
        /// Replace an AGENTS.md that was not generated by jcs (backs it up first).
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Subcommand)]
enum GatesCommand {
    /// List the gates of a project.
    List {
        /// Project name or path; defaults to the current directory.
        project: Option<String>,
    },
    /// Run the gates in the project checkout (diagnostic; tasks use a worktree).
    Run {
        /// Project name or path; defaults to the current directory.
        project: Option<String>,
        /// Run only this gate.
        #[arg(long)]
        gate: Option<String>,
    },
}

#[derive(Debug, Args)]
struct RunsArgs {
    /// Run id; omit to list recent runs.
    run: Option<String>,
}

fn main() -> Result<()> {
    // Behave like a Unix tool when the reader goes away (`jcs logs | head`): die by
    // SIGPIPE instead of panicking on a failed write to stdout.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let cli = Cli::parse();
    let socket = cli
        .socket
        .clone()
        .unwrap_or_else(jcs_proto::paths::socket_path);
    let client = Client::new(socket);

    // The TUI owns its runtime and the terminal; it must not run inside an async runtime
    // that would be starved by the input loop.
    if matches!(&cli.command, Command::Tui) {
        return crate::tui::run(client);
    }

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("cannot start the async runtime")?
        .block_on(run(cli, client))
}

async fn run(cli: Cli, client: Client) -> Result<()> {
    let json = cli.json;

    match cli.command {
        Command::Project { command } => project(&client, command, json).await,
        Command::Ask(args) => ask(&client, args, json).await,
        Command::Status => status(&client, json).await,
        Command::Logs(args) => logs(&client, args, json).await,
        Command::Runs(args) => runs(&client, args, json).await,
        Command::Index { project, force } => index(&client, project, force, json).await,
        Command::Search {
            project,
            query,
            limit,
        } => search(&client, project, query, limit, json).await,
        Command::Metrics => {
            print!("{}", client.metrics().await?);
            Ok(())
        }
        Command::Memory { project } => memory(&client, project, json).await,
        Command::Goal { goal } => show_goal(&client, &goal, json).await,
        Command::Resume { goal } => {
            let response = client.resume_goal(&goal).await?;
            println!("goal {} is now {}", response.goal_id, response.status);
            Ok(())
        }
        Command::Policy { command } => policy(&client, command, json).await,
        Command::Gates { command } => gates(&client, command, json).await,
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
        Command::Tui => unreachable!("handled before the async runtime starts"),
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
        ProjectCommand::Settings {
            project,
            auto_merge,
            budget_usd,
            budget_tokens,
            clear_budgets,
        } => {
            let projects = client.projects().await?.projects;
            let found = projects
                .iter()
                .find(|candidate| candidate.id == project || candidate.name == project)
                .ok_or_else(|| anyhow::anyhow!("no project matches {project:?}"))?;

            let settings = jcs_proto::ProjectSettings {
                auto_merge: auto_merge.unwrap_or(found.auto_merge),
                budget_usd_per_day: if clear_budgets {
                    None
                } else {
                    budget_usd.or(found.budget_usd_per_day)
                },
                budget_tokens_per_day: if clear_budgets {
                    None
                } else {
                    budget_tokens.or(found.budget_tokens_per_day)
                },
            };

            let response = client.update_project(&found.id, &settings).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                println!(
                    "{}: auto_merge={} budget_usd={} budget_tokens={}",
                    response.project.name,
                    response.project.auto_merge,
                    response
                        .project
                        .budget_usd_per_day
                        .map(|value| format!("{value:.2}"))
                        .unwrap_or_else(|| "none".into()),
                    response
                        .project
                        .budget_tokens_per_day
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "none".into())
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
        no_plan,
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

    // Capture the cursor *before* asking, so no event of this goal can be missed.
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
        plan: !no_plan,
    };
    let response = client.ask(&request).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
        if detach {
            return Ok(());
        }
    } else {
        println!(
            "goal {} · {}",
            response.goal_id,
            if response.planned {
                "planning a task DAG"
            } else {
                "single task"
            }
        );
        if detach {
            println!("detached; follow with `jcs goal {}`", response.goal_id);
            return Ok(());
        }
    }

    follow_goal(client, &response.goal_id, cursor, json).await
}

/// Follows a goal until it reaches a terminal state.
///
/// Works for both single-task and planned goals: the unit the user asked for is the goal,
/// so the unit the client follows is the goal.
async fn follow_goal(client: &Client, goal_id: &str, cursor: i64, json: bool) -> Result<()> {
    let mut stream = client.events(cursor).await?;
    let mut titles: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut task_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    while let Some(item) = stream.next().await {
        let event = item?;

        // Task-scoped events carry only a task id, so relevance is "this goal" or "one of
        // the tasks this goal created". Task creation is goal-scoped and always arrives
        // before any other event of that task.
        let belongs_to_goal = event.goal_id.as_ref().map(|goal| goal.as_str()) == Some(goal_id);
        let belongs_to_task = event
            .task_id
            .as_ref()
            .map(|task| task_ids.contains(task.as_str()))
            .unwrap_or(false);
        if !belongs_to_goal && !belongs_to_task {
            continue;
        }

        if json {
            println!("{}", serde_json::to_string(&event)?);
        }

        match event.kind {
            EventKind::TaskCreated => {
                let title = event
                    .payload
                    .get("title")
                    .and_then(|value| value.as_str())
                    .unwrap_or("task");
                if let Some(task_id) = event.task_id.as_ref() {
                    task_ids.insert(task_id.to_string());
                    titles.insert(task_id.to_string(), title.to_string());
                    if !json {
                        println!("+ task {} · {}", render::short_id(task_id.as_str()), title);
                    }
                }
            }
            EventKind::GoalStatusChanged => {
                let to = event
                    .payload
                    .get("to")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                if !json {
                    render::event(&event, true);
                }

                if matches!(to, "succeeded" | "failed" | "cancelled" | "blocked") {
                    let detail = client.goal_detail(goal_id).await?;
                    if !json {
                        println!();
                        render::goal_detail(&detail);
                    }
                    return match to {
                        "succeeded" => Ok(()),
                        "blocked" => bail!(
                            "goal blocked; inspect it with `jcs goal {goal_id}` and resume \
                             with `jcs resume {goal_id}`"
                        ),
                        other => bail!("goal ended as {other}"),
                    };
                }
            }
            EventKind::TaskStatusChanged => {
                if !json {
                    let to = event
                        .payload
                        .get("to")
                        .and_then(|value| value.as_str())
                        .unwrap_or("?");
                    let label = event
                        .task_id
                        .as_ref()
                        .map(|task| {
                            titles
                                .get(task.as_str())
                                .cloned()
                                .unwrap_or_else(|| render::short_id(task.as_str()).to_string())
                        })
                        .unwrap_or_else(|| "task".to_string());
                    println!("  {label} -> {to}");
                }
            }
            _ => {
                if !json {
                    render::event(&event, true);
                }
            }
        }
    }

    bail!("the event stream closed before the goal finished")
}

async fn show_goal(client: &Client, id: &str, json: bool) -> Result<()> {
    let detail = client.goal_detail(id).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&detail)?);
    } else {
        render::goal_detail(&detail);
    }
    Ok(())
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

async fn index(client: &Client, project: Option<String>, force: bool, json: bool) -> Result<()> {
    let project = project_or_cwd(project)?;
    let response = client.memory_index(&project, force).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
        return Ok(());
    }

    let report = &response.report;
    println!(
        "indexed {} files ({} unchanged, {} removed) · {} symbols · {} chunks · {} embeddings · {} ms",
        report.files_indexed,
        report.files_skipped,
        report.files_removed,
        report.symbols,
        report.chunks,
        report.embeddings,
        report.duration_ms
    );
    for error in report.errors.iter().take(5) {
        println!("  warning: {error}");
    }
    Ok(())
}

async fn search(
    client: &Client,
    project: Option<String>,
    query: Vec<String>,
    limit: usize,
    json: bool,
) -> Result<()> {
    let project = project_or_cwd(project)?;
    let query = query.join(" ");
    let response = client.memory_search(&project, &query, limit).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
        return Ok(());
    }

    if response.hits.is_empty() {
        println!("no results");
        return Ok(());
    }

    for hit in &response.hits {
        println!(
            "{:<32} {:<9} {:.2}  {}",
            hit.citation, hit.source, hit.score, hit.snippet
        );
    }
    Ok(())
}

async fn memory(client: &Client, project: Option<String>, json: bool) -> Result<()> {
    let project = project_or_cwd(project)?;
    let stats = client.memory_stats(&project).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&stats)?);
        return Ok(());
    }

    println!("project  {}", stats.project);
    println!(
        "files    {} · {} symbols · {} edges",
        stats.files, stats.symbols, stats.edges
    );
    println!(
        "chunks   {} · {} embedded ({})",
        stats.chunks,
        stats.embedded_chunks,
        if stats.semantic_enabled {
            "semantic search on"
        } else {
            "semantic search off"
        }
    );
    if let Some(last) = &stats.last_indexed_at {
        println!("indexed  {last}");
    } else {
        println!("indexed  never (run `jcs index`)");
    }
    Ok(())
}

async fn policy(client: &Client, command: PolicyCommand, json: bool) -> Result<()> {
    match command {
        PolicyCommand::Packs => {
            let packs = client.policy_packs().await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&packs)?);
                return Ok(());
            }
            for pack in packs {
                println!(
                    "{:<18} {:<8} {:>2} gates  {}",
                    pack.id,
                    pack.version,
                    pack.gates,
                    if pack.requires.is_empty() {
                        pack.description
                    } else {
                        format!(
                            "{} (requires: {})",
                            pack.description,
                            pack.requires.join(", ")
                        )
                    }
                );
            }
            Ok(())
        }
        PolicyCommand::Show { project } => {
            let view = client.policy(&project_or_cwd(project)?).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&view)?);
                return Ok(());
            }

            println!("project {}", view.project);
            println!(
                "packs   {}",
                view.packs
                    .iter()
                    .map(|pack| format!("{} {}", pack.id, pack.version))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            if view.drift.is_empty() {
                println!("drift   clean");
            } else {
                println!(
                    "drift   {} file(s) need `jcs policy render`",
                    view.drift.len()
                );
                for path in &view.drift {
                    println!("        {path}");
                }
            }

            println!("\nrules ({})", view.rules.len());
            for rule in &view.rules {
                println!("  - {} ({}) {}", rule.key, rule.source, rule.value);
            }

            println!("\ngates ({})", view.gates.len());
            for gate in &view.gates {
                println!(
                    "  - {:<12} {:<9} {:>4}s  {}",
                    gate.name,
                    if gate.required {
                        "required"
                    } else {
                        "optional"
                    },
                    gate.timeout_secs,
                    display_command(&gate.command)
                );
            }
            Ok(())
        }
        PolicyCommand::Render {
            project,
            check,
            force,
        } => {
            let request = jcs_proto::RenderPolicyRequest {
                project: project_or_cwd(project)?,
                check,
                force,
            };
            let response = client.render_policy(&request).await?;

            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                for file in &response.files {
                    let suffix = file
                        .detail
                        .as_ref()
                        .map(|detail| format!(" — {detail}"))
                        .unwrap_or_default();
                    println!("{:<10} {}{}", file.state, file.path, suffix);
                }
            }

            if !response.ok {
                bail!(
                    "{}",
                    if check {
                        "policy drift detected; run `jcs policy render`"
                    } else {
                        "policy render incomplete"
                    }
                );
            }
            Ok(())
        }
    }
}

async fn gates(client: &Client, command: GatesCommand, json: bool) -> Result<()> {
    match command {
        GatesCommand::List { project } => {
            let gates = client.gates(&project_or_cwd(project)?).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&gates)?);
                return Ok(());
            }
            if gates.is_empty() {
                println!("no gates: select packs in .jcs/policy.toml");
                return Ok(());
            }
            for gate in gates {
                println!(
                    "{:<12} {:<9} {:>4}s  {}",
                    gate.name,
                    if gate.required {
                        "required"
                    } else {
                        "optional"
                    },
                    gate.timeout_secs,
                    display_command(&gate.command)
                );
            }
            Ok(())
        }
        GatesCommand::Run { project, gate } => {
            let request = jcs_proto::GateRunRequest {
                project: project_or_cwd(project)?,
                gate,
            };
            let response = client.run_gates(&request).await?;

            if json {
                println!("{}", serde_json::to_string_pretty(&response)?);
            } else {
                eprintln!("note: gates ran in the project checkout, not in a task worktree");
                for outcome in &response.outcomes {
                    println!(
                        "{:<12} {:<8} {:>7} ms{}",
                        outcome.name,
                        outcome.status,
                        outcome.duration_ms,
                        outcome
                            .exit_code
                            .map(|code| format!("  exit {code}"))
                            .unwrap_or_default()
                    );
                    for finding in outcome.findings.iter().take(5) {
                        println!("    {finding}");
                    }
                    // A failing gate with no parsed findings still has output, and that
                    // output is often the only thing that says what to fix.
                    if outcome.status != "passed" && outcome.findings.is_empty() {
                        for line in outcome.excerpt.lines().take(10) {
                            println!("    {line}");
                        }
                    }
                }
            }

            if !response.ok {
                bail!("one or more required gates failed");
            }
            Ok(())
        }
    }
}

fn project_or_cwd(project: Option<String>) -> Result<String> {
    match project {
        Some(project) => Ok(project),
        None => Ok(std::env::current_dir()
            .context("cannot determine the current directory")?
            .to_string_lossy()
            .to_string()),
    }
}

fn display_command(command: &[String]) -> String {
    command
        .iter()
        .map(|part| {
            if part.contains(' ') {
                format!("{part:?}")
            } else {
                part.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
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
