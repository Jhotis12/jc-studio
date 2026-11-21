//! Window layout.
//!
//! Ninety percent of the interface is a projection of the daemon snapshot: projects on the
//! left, the selected goal on the right, and the workspace in the middle. Everything that
//! changes durable state is an explicit button, never a text command.

pub mod board;
pub mod code;
pub mod dag;
pub mod details;
pub mod extensions;
pub mod memory;
pub mod policy;
pub mod settings;
pub mod system;
pub mod trace;

use egui::{Color32, RichText};

use crate::app::{Studio, Tab};
use crate::state::{Connection, Notice};

/// Renders modal windows. Kept separate so the shell never owns dialog state.
pub fn windows(context: &egui::Context, app: &mut Studio) {
    settings::windows(context, app);
}

/// Renders the persistent shell around the workspace.
pub fn shell(context: &egui::Context, app: &mut Studio) {
    let width = context.screen_rect().width();
    app.wide = width >= 1000.0;
    let show_projects = width >= 640.0;

    top_bar(context, app);
    console(context, app);
    if show_projects {
        projects(context, app);
    }
    if app.wide {
        details(context, app);
    }
    central(context, app, show_projects);
}

fn top_bar(context: &egui::Context, app: &mut Studio) {
    egui::TopBottomPanel::top("top-bar")
        .exact_height(46.0)
        .show(context, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("JC Studio").heading().strong());
                ui.separator();
                connection_badge(ui, &app.state);
                ui.separator();
                // The top bar stays narrow on purpose: version, uptime and paths live in
                // Sistema, where there is room to read them.
                match &app.state.health {
                    Some(health) if !health.agent.reachable => {
                        ui.colored_label(Color32::from_rgb(230, 170, 90), "agente no disponible");
                    }
                    Some(_) => {}
                    None => {
                        ui.label(RichText::new("sin daemon").weak());
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let connected = app.state.is_connected();
                    ui.add_enabled_ui(connected, |ui| {
                        if ui.selectable_label(app.show_console, "Consola").clicked() {
                            app.show_console = !app.show_console;
                        }
                    });
                    if ui.button("Ajustes").clicked() {
                        app.open_settings();
                    }
                    if ui
                        .small_button("↻")
                        .on_hover_text("Actualizar desde el daemon")
                        .clicked()
                    {
                        app.daemon.send(crate::daemon::Command::Refresh);
                    }
                    ui.separator();
                    ui.add_enabled(
                        app.state.is_connected(),
                        egui::Label::new(
                            RichText::new(format!("${:.4}", app.state.totals.cost_usd)).strong(),
                        ),
                    )
                    .on_hover_text("coste acumulado de todos los proyectos");
                });
            });
        });
}

fn connection_badge(ui: &mut egui::Ui, state: &crate::state::State) {
    let (color, text) = match &state.connection {
        Some(Connection::Online) => (Color32::from_rgb(120, 200, 130), "conectado"),
        Some(Connection::Offline(_)) => (Color32::from_rgb(225, 110, 110), "sin conexión"),
        Some(Connection::Connecting) | None => (Color32::from_rgb(210, 190, 110), "conectando"),
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, color);
    ui.label(text);
    if let Some(Connection::Offline(message)) = &state.connection {
        response.on_hover_text(message);
    }
}

fn projects(context: &egui::Context, app: &mut Studio) {
    egui::SidePanel::left("projects")
        .resizable(true)
        .default_width(250.0)
        .min_width(180.0)
        .show(context, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("Proyectos");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("+ Añadir").clicked() {
                        app.show_add_project = true;
                    }
                });
            });
            ui.separator();

            if app.state.projects.is_empty() {
                ui.add_space(8.0);
                ui.label("Ningún repositorio registrado.");
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "Añade un proyecto con su ruta y el daemon lo indexará y ejecutará los \
                         objetivos allí.",
                    )
                    .weak(),
                );
                return;
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                let projects: Vec<_> = app.state.projects.clone();
                for project in &projects {
                    let selected = app.state.selected_project.as_deref() == Some(&project.id);
                    let goals = app
                        .state
                        .goals
                        .iter()
                        .filter(|goal| goal.project_id == project.id)
                        .count();
                    let running = app
                        .state
                        .goals
                        .iter()
                        .any(|goal| goal.project_id == project.id && goal.status == "running");

                    if ui
                        .selectable_label(selected, RichText::new(&project.name).strong())
                        .clicked()
                    {
                        app.select_project(project.id.clone());
                    }
                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        if running {
                            ui.colored_label(
                                Color32::from_rgb(110, 170, 235),
                                format!("● {goals} objetivos"),
                            );
                        } else {
                            ui.label(RichText::new(format!("{goals} objetivos")).weak());
                        }
                        if project.auto_merge {
                            ui.label(RichText::new("auto-merge").weak());
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        ui.label(RichText::new(&project.root_path).weak().small());
                    });
                    ui.add_space(4.0);
                }
            });
        });
}

fn details(context: &egui::Context, app: &mut Studio) {
    egui::SidePanel::right("details")
        .resizable(true)
        .default_width(390.0)
        .min_width(280.0)
        .show(context, |ui| {
            ui.add_space(6.0);
            ui.heading("Detalle");
            ui.separator();
            details::goal(ui, app);
        });
}

fn central(context: &egui::Context, app: &mut Studio, show_projects: bool) {
    egui::CentralPanel::default().show(context, |ui| {
        if let Some(notice) = app.state.notice.clone() {
            notice_bar(ui, app, notice);
            ui.add_space(4.0);
        }

        if !app.state.is_connected() {
            offline(ui, app);
            return;
        }

        if app.state.projects.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.heading("JC Studio");
                ui.label("Un plano de control para agentes que trabajan sobre tus repositorios.");
                ui.add_space(12.0);
                ui.label(
                    "Empieza añadiendo un proyecto: el daemon creará worktrees, ejecutará los \
                     objetivos y verificará cada cambio con gates antes de integrarlo.",
                );
            });
            return;
        }

        ui.horizontal_wrapped(|ui| {
            if !show_projects {
                // Narrow window: the project list becomes a selector instead of a panel.
                let current = app
                    .state
                    .project()
                    .map(|project| project.name.clone())
                    .unwrap_or_else(|| "proyecto".to_string());
                egui::ComboBox::from_id_salt("project-selector")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        let projects: Vec<_> = app.state.projects.clone();
                        for project in projects {
                            if ui
                                .selectable_label(
                                    app.state.selected_project.as_deref() == Some(&project.id),
                                    &project.name,
                                )
                                .clicked()
                            {
                                app.select_project(project.id.clone());
                            }
                        }
                    });
                ui.separator();
            }
            ui.selectable_value(&mut app.tab, Tab::Goals, "Objetivos");
            ui.selectable_value(&mut app.tab, Tab::Code, "Código");
            if !app.wide {
                ui.selectable_value(&mut app.tab, Tab::Details, "Detalle");
            }
            ui.selectable_value(&mut app.tab, Tab::Extensions, "Skills/MCP");
            ui.selectable_value(&mut app.tab, Tab::Policy, "Política");
            ui.selectable_value(&mut app.tab, Tab::Memory, "Memoria");
            ui.selectable_value(&mut app.tab, Tab::System, "Sistema");
        });
        ui.horizontal(|ui| {
            let active = app
                .state
                .goals
                .iter()
                .filter(|goal| matches!(goal.status.as_str(), "pending" | "running"))
                .count();
            let working = app
                .state
                .tasks
                .iter()
                .filter(|task| matches!(task.status.as_str(), "running" | "verifying"))
                .count();
            ui.label(
                RichText::new(format!(
                    "{active} objetivos activos · {working} tareas en curso"
                ))
                .weak()
                .small(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(project) = app.state.project() {
                    ui.label(RichText::new(&project.base_branch).weak().monospace());
                    ui.label(RichText::new("rama base").weak());
                }
            });
        });
        ui.separator();

        match app.tab {
            Tab::Goals => board::goals(ui, app),
            Tab::Code => code::panel(ui, app),
            Tab::Details => details::goal(ui, app),
            Tab::Extensions => extensions::panel(ui, app),
            Tab::Policy => policy::panel(ui, app),
            Tab::Memory => memory::panel(ui, app),
            Tab::System => system::panel(ui, app),
        }
    });
}

/// Shown instead of the workspace while the daemon is unreachable. Starting the service is
/// the one action a new installation needs, and it should not require a terminal.
fn offline(ui: &mut egui::Ui, app: &mut Studio) {
    ui.add_space(30.0);
    ui.vertical_centered(|ui| match &app.state.connection {
        Some(Connection::Offline(message)) => {
            ui.heading("Sin conexión con el daemon");
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "JC Studio necesita jcsd para leer objetivos y ejecutar trabajo. La app \
                         reintenta cada dos segundos.",
                )
                .weak(),
            );
            ui.add_space(10.0);
            ui.label(RichText::new(message).monospace().small().weak());
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui.button("Arrancar daemon").clicked() {
                    match start_daemon() {
                        Ok(message) => {
                            app.state.notice = Some(Notice {
                                message,
                                error: false,
                            });
                        }
                        Err(message) => {
                            app.state.notice = Some(Notice {
                                message,
                                error: true,
                            });
                        }
                    }
                }
                if ui.button("Reintentar ahora").clicked() {
                    app.daemon.send(crate::daemon::Command::Refresh);
                }
            });
            ui.add_space(12.0);
            ui.label(
                RichText::new(
                    "Si no está instalado como servicio: `systemctl --user enable --now jcsd`. \
                         Para ver los logs: `journalctl --user -u jcsd -f`.",
                )
                .weak()
                .small(),
            );
        }
        _ => {
            ui.spinner();
            ui.add_space(6.0);
            ui.label("Conectando con el daemon…");
        }
    });
}

/// Tries the user service first, then a `jcsd` binary next to the application or on PATH.
fn start_daemon() -> Result<String, String> {
    if let Ok(status) = std::process::Command::new("systemctl")
        .args(["--user", "start", "jcsd"])
        .status()
    {
        if status.success() {
            return Ok("jcsd arrancado con systemd".to_string());
        }
    }

    let mut candidates = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join("jcsd"));
        }
    }
    candidates.push(std::path::PathBuf::from("jcsd"));

    for candidate in candidates {
        if let Ok(child) = std::process::Command::new(&candidate).spawn() {
            return Ok(format!("jcsd arrancado (pid {})", child.id()));
        }
    }

    Err(
        "no se pudo arrancar jcsd: instala el servicio y ejecuta `systemctl --user start jcsd`"
            .to_string(),
    )
}

fn notice_bar(ui: &mut egui::Ui, app: &mut Studio, notice: Notice) {
    let color = if notice.error {
        Color32::from_rgb(225, 110, 110)
    } else {
        Color32::from_rgb(120, 200, 130)
    };
    egui::Frame::group(ui.style())
        .fill(color.gamma_multiply(0.12))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(color, if notice.error { "Error" } else { "Hecho" });
                ui.label(&notice.message);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("×").clicked() {
                        app.state.notice = None;
                    }
                });
            });
        });
}

fn console(context: &egui::Context, app: &mut Studio) {
    if !app.show_console {
        return;
    }
    egui::TopBottomPanel::bottom("console")
        .resizable(true)
        .default_height(240.0)
        .min_height(120.0)
        .show(context, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut app.console_tab,
                    crate::app::ConsoleTab::Events,
                    "Eventos",
                );
                ui.selectable_value(&mut app.console_tab, crate::app::ConsoleTab::Agent, "Chat");
                let label = if app.terminals.is_empty() {
                    "Terminal".to_string()
                } else {
                    format!("Terminal · {}", app.terminals.len())
                };
                ui.selectable_value(
                    &mut app.console_tab,
                    crate::app::ConsoleTab::Terminals,
                    label,
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("×").clicked() {
                        app.show_console = false;
                    }
                });
            });
            ui.separator();
            match app.console_tab {
                crate::app::ConsoleTab::Events => events(ui, app),
                crate::app::ConsoleTab::Agent => trace::panel(ui, app),
                crate::app::ConsoleTab::Terminals => {
                    let cwd = app
                        .state
                        .project()
                        .map(|project| std::path::PathBuf::from(&project.root_path));
                    app.terminals.ui(ui, cwd);
                }
            }
        });
}

fn events(ui: &mut egui::Ui, app: &mut Studio) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.event_filter)
                .hint_text("filtrar eventos")
                .desired_width(220.0),
        );
        if ui.small_button("Limpiar").clicked() {
            app.state.events.clear();
        }
        if app.state.selected_goal().is_some() {
            ui.checkbox(&mut app.events_for_goal, "solo este objetivo");
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let count = app.state.events.len();
            ui.label(
                RichText::new(if count == 1 {
                    "1 evento".to_string()
                } else {
                    format!("{count} eventos")
                })
                .weak()
                .small(),
            );
        });
    });
    ui.add_space(2.0);

    let filter = app.event_filter.to_lowercase();
    let only_goal = if app.events_for_goal {
        app.state.selected_goal.clone()
    } else {
        None
    };
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(true)
        .show(ui, |ui| {
            let mut shown = 0usize;
            for line in &app.state.events {
                if !filter.is_empty()
                    && !line.kind.to_lowercase().contains(&filter)
                    && !line.detail.to_lowercase().contains(&filter)
                {
                    continue;
                }
                if let Some(goal) = &only_goal {
                    if line.goal_id.as_deref() != Some(goal.as_str()) {
                        continue;
                    }
                }
                shown += 1;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&line.time).monospace().weak().small());
                    let color = match line.kind.as_str() {
                        "error" => Color32::from_rgb(225, 110, 110),
                        "run_finished" | "commit_created" | "merge_finished" => {
                            Color32::from_rgb(120, 200, 130)
                        }
                        "agent_text" => Color32::from_rgb(150, 170, 210),
                        _ => Color32::from_rgb(180, 180, 190),
                    };
                    ui.label(RichText::new(&line.kind).monospace().small().color(color));
                    if !line.detail.is_empty() {
                        ui.add(
                            egui::Label::new(
                                RichText::new(truncate(&line.detail, 160))
                                    .monospace()
                                    .small(),
                            )
                            .truncate(),
                        )
                        .on_hover_text(&line.detail);
                    }
                });
            }
            if shown == 0 {
                ui.label(
                    RichText::new(if only_goal.is_some() {
                        "Sin actividad registrada para este objetivo."
                    } else {
                        "Sin eventos que coincidan con el filtro."
                    })
                    .weak(),
                );
            }
        });
}

/// Standard status colour, shared by every panel so the same state never means two colours.
pub fn status_color(status: &str) -> Color32 {
    match status {
        "succeeded" => Color32::from_rgb(110, 200, 130),
        "failed" | "timed_out" | "interrupted" => Color32::from_rgb(225, 110, 110),
        "blocked" => Color32::from_rgb(230, 170, 90),
        "running" | "verifying" | "started" => Color32::from_rgb(105, 165, 235),
        "cancelled" => Color32::from_rgb(150, 150, 160),
        _ => Color32::from_rgb(170, 170, 180),
    }
}

pub fn badge(ui: &mut egui::Ui, status: &str) {
    let color = status_color(status);
    egui::Frame::new()
        .fill(color.gamma_multiply(0.16))
        .inner_margin(egui::Margin::symmetric(5, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(status).small().color(color).strong());
        });
}

/// A text field plus a catalog picker. Free text stays possible for models the backend
/// does not advertise (an alias, a proxy, a model added after the last connection).
pub fn model_selector(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut String,
    models: &[jcs_proto::ModelCatalogEntry],
) {
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text("provider/modelo")
            .desired_width(190.0),
    );
    egui::ComboBox::from_id_salt(id)
        .selected_text("elegir")
        .width(240.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(value, String::new(), "sin modelo (lo decide el backend)");
            for model in models {
                let label = format!(
                    "{}  ·  ${:.2} in / ${:.2} out",
                    model.reference, model.cost_input, model.cost_output
                );
                ui.selectable_value(value, model.reference.clone(), label)
                    .on_hover_text(&model.name);
            }
        });
}

/// Truncates on a character boundary; slicing bytes can split a multi-byte character.
pub fn truncate(text: &str, limit: usize) -> String {
    let mut out = String::new();
    for (index, character) in text.chars().enumerate() {
        if index >= limit {
            out.push('…');
            break;
        }
        out.push(character);
    }
    out
}

/// Four segments and the stage name. No numeric percentage: a task does not have a
/// meaningful "% done" while the model is thinking.
pub fn stage_steps(ui: &mut egui::Ui, progress: crate::state::TaskProgress, width: f32) {
    let color = status_color(progress.stage.status_key());
    ui.horizontal(|ui| {
        let segment = (width / progress.total.max(1) as f32).max(4.0);
        for index in 0..progress.total {
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2((segment - 2.0).max(2.0), 5.0),
                egui::Sense::hover(),
            );
            let fill = if index < progress.filled {
                color
            } else {
                egui::Color32::from_gray(48)
            };
            ui.painter().rect_filled(rect, 2.0, fill);
        }
        ui.label(RichText::new(progress.stage.label()).small().color(color));
        if progress.stage.active() {
            ui.spinner();
        }
    });
}

/// One equal segment per task, colored by stage. This is the truthful aggregate: a single
/// fill turned blue for the whole bar as soon as any task was running.
pub fn task_strip(ui: &mut egui::Ui, tasks: &[jcs_proto::TaskView]) {
    if tasks.is_empty() {
        return;
    }
    let width = ui.available_width().max(80.0);
    let segment = width / tasks.len() as f32;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 8.0), egui::Sense::hover());
    for (index, task) in tasks.iter().enumerate() {
        let progress = crate::state::task_progress(task);
        let color = status_color(progress.stage.status_key());
        let piece = egui::Rect::from_min_size(
            egui::pos2(rect.left() + index as f32 * segment, rect.top()),
            egui::vec2((segment - 2.0).max(1.0), 8.0),
        );
        ui.painter().rect_filled(piece, 2.0, color);
    }
}

pub fn metric(ui: &mut egui::Ui, label: &str, value: impl Into<String>) {
    ui.vertical(|ui| {
        ui.label(RichText::new(value.into()).strong().size(16.0));
        ui.label(RichText::new(label).weak().small());
    });
}
