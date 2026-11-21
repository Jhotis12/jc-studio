//! Background connection between the desktop application and `jcsd`.
//!
//! The UI thread must never block on I/O or hold a socket. A dedicated thread owns a
//! single-threaded Tokio runtime, sends updates through a channel, and reconnects with
//! backoff. The UI polls the channel once per frame; it never awaits.
//!
//! Slow operations (running gates, indexing a repository) are executed in their own tasks
//! so a five-minute `cargo test` gate cannot freeze status updates or the event stream.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use jcs_client::Client;
use jcs_core::Event;
use jcs_proto::{
    AddProjectRequest, AskRequest, GateRunRequest, GateRunResponse, GoalActionResponse,
    HealthResponse, MemoryIndexResponse, MemorySearchResponse, MemoryStatsView, PermissionMode,
    PolicyView, ProjectSettings, ProjectView, RenderPolicyRequest, RenderPolicyResponse,
    RunDetailResponse, StatusResponse,
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

/// Requests the interface can send to the daemon.
#[derive(Debug, Clone)]
pub enum Command {
    /// Fetch an authoritative snapshot now.
    Refresh,
    LoadSettings,
    SaveSettings(jcs_proto::SettingsPatch),
    Ask {
        project: String,
        prompt: String,
        model: Option<String>,
        plan: bool,
    },
    Resume {
        goal: String,
    },
    Abort {
        run: String,
    },
    AddProject {
        path: String,
        name: Option<String>,
        base_branch: Option<String>,
    },
    UpdateProject {
        id: String,
        settings: ProjectSettings,
    },
    RemoveProject {
        id: String,
    },
    Policy {
        project: String,
    },
    RenderPolicy {
        project: String,
        check: bool,
    },
    RunGates {
        project: String,
        gate: Option<String>,
    },
    IndexMemory {
        project: String,
        force: bool,
    },
    MemoryStats {
        project: String,
    },
    SearchMemory {
        project: String,
        query: String,
    },
    RunDetail {
        run: String,
    },
    TaskEvents {
        task: String,
    },
}

/// Facts the daemon reports back to the interface.
#[derive(Debug)]
pub enum Update {
    Online(HealthResponse),
    Offline(String),
    Status(StatusResponse),
    History(Vec<Event>),
    Event(Event),
    Notice { message: String, error: bool },
    Policy(PolicyView),
    Rendered(RenderPolicyResponse),
    Gates(GateRunResponse),
    MemoryIndex(MemoryIndexResponse),
    MemoryStats(MemoryStatsView),
    MemorySearch(MemorySearchResponse),
    RunDetail(Box<RunDetailResponse>),
    TaskHistory(Vec<Event>),
    GoalAction(GoalActionResponse),
    Project(ProjectView),
    Settings(jcs_proto::SettingsView),
    Models(Vec<jcs_proto::ModelCatalogEntry>),
}

/// Handle owned by the interface.
pub struct Daemon {
    commands: UnboundedSender<Command>,
    updates: Receiver<Update>,
}

impl Daemon {
    /// Starts the worker thread. The socket is read at spawn time so `JCS_SOCKET` works.
    pub fn spawn(socket: PathBuf) -> Self {
        let (commands, command_rx) = unbounded_channel();
        let (update_tx, updates) = std::sync::mpsc::channel();

        std::thread::Builder::new()
            .name("jcs-daemon-connection".to_string())
            .spawn(move || worker(socket, command_rx, update_tx))
            .expect("cannot start the daemon connection thread");

        Self { commands, updates }
    }

    pub fn send(&self, command: Command) {
        // A closed channel only happens while shutting down; losing a click then is fine.
        let _ = self.commands.send(command);
    }

    /// Drains everything that arrived since the previous frame.
    pub fn poll(&self) -> Vec<Update> {
        self.updates.try_iter().collect()
    }
}

fn worker(socket: PathBuf, commands: UnboundedReceiver<Command>, updates: Sender<Update>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("cannot build the daemon runtime");

    runtime.block_on(connection_loop(socket, commands, updates));
}

async fn connection_loop(
    socket: PathBuf,
    mut commands: UnboundedReceiver<Command>,
    updates: Sender<Update>,
) {
    let client = Client::new(&socket);
    let mut cursor = 0i64;
    let mut seeded = false;
    let mut offline_reported: Option<String> = None;

    loop {
        let health = match client.health().await {
            Ok(health) => health,
            Err(error) => {
                let message = error.to_string();
                if offline_reported.as_deref() != Some(message.as_str()) {
                    let _ = updates.send(Update::Offline(message.clone()));
                    offline_reported = Some(message);
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        offline_reported = None;
        let _ = updates.send(Update::Online(health));

        // The catalog changes rarely; refresh it on every (re)connection instead of
        // asking on each frame.
        if let Ok(models) = client.models().await {
            let _ = updates.send(Update::Models(models));
        }
        // The settings carry the effective concurrency, used to explain why a pending task
        // is not running yet.
        if let Ok(settings) = client.settings().await {
            let _ = updates.send(Update::Settings(settings));
        }

        if !seeded {
            if let Ok(history) = client.event_history(None, 200).await {
                cursor = history.last().map(|event| event.seq).unwrap_or(cursor);
                let _ = updates.send(Update::History(history));
                seeded = true;
            }
        }

        refresh(&client, &updates).await;

        let mut stream = match client.events(cursor).await {
            Ok(stream) => stream,
            Err(error) => {
                let _ = updates.send(Update::Offline(error.to_string()));
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        let mut ticker = tokio::time::interval(Duration::from_secs(2));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                event = stream.next() => {
                    match event {
                        Some(Ok(event)) => {
                            cursor = cursor.max(event.seq);
                            let _ = updates.send(Update::Event(event));
                        }
                        Some(Err(error)) => {
                            let _ = updates.send(Update::Offline(error.to_string()));
                            break;
                        }
                        None => {
                            let _ = updates.send(Update::Offline("the event stream closed".to_string()));
                            break;
                        }
                    }
                }
                command = commands.recv() => {
                    match command {
                        Some(Command::Refresh) => refresh(&client, &updates).await,
                        Some(command) => handle(&client, &updates, command).await,
                        None => return,
                    }
                }
                _ = ticker.tick() => refresh(&client, &updates).await,
            }
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn refresh(client: &Client, updates: &Sender<Update>) {
    if let Ok(status) = client.status().await {
        let _ = updates.send(Update::Status(status));
    }
}

async fn handle(client: &Client, updates: &Sender<Update>, command: Command) {
    match command {
        Command::Refresh => refresh(client, updates).await,
        Command::LoadSettings => match client.settings().await {
            Ok(settings) => {
                let _ = updates.send(Update::Settings(settings));
            }
            Err(error) => notice(updates, error, "no se pudieron leer los ajustes"),
        },
        Command::SaveSettings(patch) => match client.update_settings(&patch).await {
            Ok(settings) => {
                let restart = settings.restart_required.join(", ");
                let message = if restart.is_empty() {
                    "ajustes guardados".to_string()
                } else {
                    format!("ajustes guardados; requieren reiniciar el daemon: {restart}")
                };
                let _ = updates.send(Update::Settings(settings));
                let _ = updates.send(Update::Notice {
                    message,
                    error: false,
                });
            }
            Err(error) => notice(updates, error, "no se pudieron guardar los ajustes"),
        },
        Command::Ask {
            project,
            prompt,
            model,
            plan,
        } => {
            let request = AskRequest {
                project,
                prompt,
                model,
                agent: None,
                permissions: PermissionMode::Auto,
                title: None,
                plan,
            };
            match client.ask(&request).await {
                Ok(response) => {
                    let message = if response.planned {
                        format!("objetivo {} en planificación", short(&response.goal_id))
                    } else {
                        format!("objetivo {} encolado", short(&response.goal_id))
                    };
                    let _ = updates.send(Update::Notice {
                        message,
                        error: false,
                    });
                    refresh(client, updates).await;
                }
                Err(error) => notice(updates, error, "no se pudo encolar el objetivo"),
            }
        }
        Command::Resume { goal } => match client.resume_goal(&goal).await {
            Ok(response) => {
                let _ = updates.send(Update::GoalAction(response));
                refresh(client, updates).await;
            }
            Err(error) => notice(updates, error, "no se pudo reanudar el objetivo"),
        },
        Command::Abort { run } => match client.abort(&run).await {
            Ok(_) => {
                let _ = updates.send(Update::Notice {
                    message: format!("run {} abortado", short(&run)),
                    error: false,
                });
                refresh(client, updates).await;
            }
            Err(error) => notice(updates, error, "no se pudo abortar el run"),
        },
        Command::AddProject {
            path,
            name,
            base_branch,
        } => {
            let request = AddProjectRequest {
                path,
                name,
                base_branch,
            };
            match client.add_project(&request).await {
                Ok(response) => {
                    let _ = updates.send(Update::Project(response.project));
                    let _ = updates.send(Update::Notice {
                        message: "proyecto añadido".to_string(),
                        error: false,
                    });
                    refresh(client, updates).await;
                }
                Err(error) => notice(updates, error, "no se pudo añadir el proyecto"),
            }
        }
        Command::UpdateProject { id, settings } => {
            match client.update_project(&id, &settings).await {
                Ok(response) => {
                    let _ = updates.send(Update::Project(response.project));
                    let _ = updates.send(Update::Notice {
                        message: "ajustes del proyecto guardados".to_string(),
                        error: false,
                    });
                    refresh(client, updates).await;
                }
                Err(error) => notice(updates, error, "no se pudieron guardar los ajustes"),
            }
        }
        Command::RemoveProject { id } => match client.remove_project(&id).await {
            Ok(_) => {
                let _ = updates.send(Update::Notice {
                    message: "proyecto eliminado".to_string(),
                    error: false,
                });
                refresh(client, updates).await;
            }
            Err(error) => notice(updates, error, "no se pudo eliminar el proyecto"),
        },
        Command::Policy { project } => match client.policy(&project).await {
            Ok(policy) => {
                let _ = updates.send(Update::Policy(policy));
            }
            Err(error) => notice(updates, error, "no se pudo leer la política"),
        },
        Command::RenderPolicy { project, check } => {
            let client = client.clone();
            let updates = updates.clone();
            tokio::spawn(async move {
                let request = RenderPolicyRequest {
                    project,
                    check,
                    force: false,
                };
                match client.render_policy(&request).await {
                    Ok(response) => {
                        let _ = updates.send(Update::Rendered(response));
                    }
                    Err(error) => notice(&updates, error, "no se pudo renderizar la política"),
                }
            });
        }
        Command::RunGates { project, gate } => {
            let client = client.clone();
            let updates = updates.clone();
            tokio::spawn(async move {
                let request = GateRunRequest { project, gate };
                match client.run_gates(&request).await {
                    Ok(response) => {
                        let _ = updates.send(Update::Gates(response));
                    }
                    Err(error) => notice(&updates, error, "no se pudieron ejecutar los gates"),
                }
            });
        }
        Command::IndexMemory { project, force } => {
            let client = client.clone();
            let updates = updates.clone();
            tokio::spawn(async move {
                match client.memory_index(&project, force).await {
                    Ok(response) => {
                        let _ = updates.send(Update::MemoryIndex(response));
                        if let Ok(stats) = client.memory_stats(&project).await {
                            let _ = updates.send(Update::MemoryStats(stats));
                        }
                    }
                    Err(error) => notice(&updates, error, "no se pudo indexar el proyecto"),
                }
            });
        }
        Command::MemoryStats { project } => match client.memory_stats(&project).await {
            Ok(stats) => {
                let _ = updates.send(Update::MemoryStats(stats));
            }
            Err(error) => notice(updates, error, "no se pudo leer la memoria"),
        },
        Command::SearchMemory { project, query } => {
            let client = client.clone();
            let updates = updates.clone();
            tokio::spawn(async move {
                match client.memory_search(&project, &query, 20).await {
                    Ok(response) => {
                        let _ = updates.send(Update::MemorySearch(response));
                    }
                    Err(error) => notice(&updates, error, "no se pudo buscar en la memoria"),
                }
            });
        }
        Command::TaskEvents { task } => match client.task_events(&task, 500).await {
            Ok(events) => {
                let _ = updates.send(Update::TaskHistory(events));
            }
            Err(error) => notice(updates, error, "no se pudo leer la actividad de la tarea"),
        },
        Command::RunDetail { run } => match client.run_detail(&run).await {
            Ok(detail) => {
                let _ = updates.send(Update::RunDetail(Box::new(detail)));
            }
            Err(error) => notice(updates, error, "no se pudo leer el run"),
        },
    }
}

fn notice(updates: &Sender<Update>, error: anyhow::Error, context: &str) {
    let _ = updates.send(Update::Notice {
        message: format!("{context}: {error}"),
        error: true,
    });
}

/// Short form of an identifier for the interface. The full id stays in the data model.
pub fn short(id: &str) -> &str {
    let without_prefix = id.split_once('_').map(|(_, rest)| rest).unwrap_or(id);
    &without_prefix[..without_prefix.len().min(8)]
}

#[cfg(test)]
mod tests {
    use super::short;

    #[test]
    fn identifiers_are_shortened_for_display() {
        assert_eq!(short("goal_0123456789abcdef"), "01234567");
        assert_eq!(short("raw"), "raw");
    }
}
