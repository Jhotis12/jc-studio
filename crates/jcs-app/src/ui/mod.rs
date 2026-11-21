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
pub mod markdown;
pub mod memory;
pub mod orchestrator;
pub mod policy;
pub mod settings;
pub mod system;
pub mod theme;
pub mod trace;

use egui::{Color32, RichText};

use crate::app::{Studio, Tab};
use crate::state::{Connection, Notice};

/// Renders modal windows. Kept separate so the shell never owns dialog state.
pub fn windows(context: &egui::Context, app: &mut Studio) {
    settings::windows(context, app);
    orchestrator::windows(context, app);
}

/// Renders the persistent shell around the workspace.
pub fn shell(context: &egui::Context, app: &mut Studio) {
    let width = context.screen_rect().width();
    // Three panels need real width for the workspace. Below this the detail becomes a tab,
    // which keeps the code/board usable instead of squeezing it between two sidebars.
    app.wide = width >= 1160.0;
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
        .exact_height(56.0)
        .frame(
            egui::Frame::new()
                .fill(theme::SURFACE)
                .inner_margin(egui::Margin::symmetric(14, 0))
                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER)),
        )
        .show(context, |ui| {
            ui.horizontal_centered(|ui| {
                let (mark, _) =
                    ui.allocate_exact_size(egui::vec2(27.0, 27.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(mark, egui::CornerRadius::same(8), theme::ACCENT);
                ui.painter().text(
                    mark.center(),
                    egui::Align2::CENTER_CENTER,
                    "JC",
                    egui::FontId::proportional(11.0),
                    theme::BG,
                );
                ui.add_space(2.0);
                ui.label(
                    RichText::new("JC Studio")
                        .size(16.0)
                        .strong()
                        .color(theme::TEXT),
                );
                ui.add_space(8.0);
                connection_badge(ui, &app.state);
                // The top bar stays narrow on purpose: version, uptime and paths live in
                // Sistema, where there is room to read them.
                match &app.state.health {
                    Some(health) if !health.agent.reachable => {
                        health_pill(ui, "agente no disponible", theme::WARNING);
                    }
                    Some(_) => {}
                    None => {
                        health_pill(ui, "sin daemon", theme::FAINT);
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let connected = app.state.is_connected();
                    let console_label = RichText::new("Consola").size(12.5);
                    if ui
                        .add_enabled(
                            connected,
                            egui::Button::new(console_label)
                                .selected(app.show_console)
                                .min_size(egui::vec2(0.0, 30.0)),
                        )
                        .clicked()
                    {
                        app.show_console = !app.show_console;
                    }
                    if ui
                        .add(egui::Button::new(RichText::new("Ajustes").size(12.5)))
                        .clicked()
                    {
                        app.open_settings();
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("↻").size(14.0))
                                .min_size(egui::vec2(30.0, 30.0)),
                        )
                        .on_hover_text("Actualizar desde el daemon")
                        .clicked()
                    {
                        app.daemon.send(crate::daemon::Command::Refresh);
                    }
                    let cost = theme::pill(theme::SURFACE_RAISED)
                        .show(ui, |ui| {
                            // The top bar is laid out right-to-left; add the amount first so
                            // it reads "gasto $0.0000" visually.
                            ui.label(
                                RichText::new(format!("${:.4}", app.state.totals.cost_usd))
                                    .size(13.0)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            ui.label(RichText::new("gasto").size(11.0).color(theme::FAINT));
                        })
                        .response;
                    cost.on_hover_text(
                        "coste de los runs de tareas, calculado con el catálogo si el \
                         backend reporta 0; el planificador y el revisor aún no se \
                         contabilizan",
                    );
                });
            });
        });
}

fn connection_badge(ui: &mut egui::Ui, state: &crate::state::State) {
    let (color, text) = match &state.connection {
        Some(Connection::Online) => (theme::SUCCESS, "conectado"),
        Some(Connection::Offline(_)) => (theme::DANGER, "sin conexión"),
        Some(Connection::Connecting) | None => (theme::WARNING, "conectando"),
    };
    let response = theme::pill(color.gamma_multiply(0.14))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(7.0, 7.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 3.5, color);
                ui.label(RichText::new(text).size(12.0).color(color));
            });
        })
        .response;
    if let Some(Connection::Offline(message)) = &state.connection {
        response.on_hover_text(message);
    }
}

fn health_pill(ui: &mut egui::Ui, text: &str, color: Color32) {
    theme::pill(color.gamma_multiply(0.12)).show(ui, |ui| {
        ui.label(RichText::new(text).size(11.5).color(color));
    });
}

fn projects(context: &egui::Context, app: &mut Studio) {
    egui::SidePanel::left("projects")
        .resizable(true)
        .default_width(252.0)
        .min_width(200.0)
        .max_width(330.0)
        .frame(
            egui::Frame::new()
                .fill(theme::SURFACE)
                .inner_margin(egui::Margin::symmetric(12, 10))
                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER)),
        )
        .show(context, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Proyectos")
                            .size(18.0)
                            .strong()
                            .color(theme::TEXT),
                    );
                    ui.label(theme::muted("repositorios bajo control"));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("+ Añadir")
                                    .size(12.0)
                                    .strong()
                                    .color(theme::BG),
                            )
                            .fill(theme::ACCENT)
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(egui::CornerRadius::same(6))
                            .min_size(egui::vec2(0.0, 28.0)),
                        )
                        .clicked()
                    {
                        app.show_add_project = true;
                    }
                });
            });
            ui.add_space(10.0);

            if app.state.projects.is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new("Ningún repositorio registrado.").size(14.0));
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "Añade un proyecto con su ruta y el daemon lo indexará y ejecutará los \
                         objetivos allí.",
                    )
                    .size(12.5)
                    .color(theme::MUTED),
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

                    let card = theme::card(selected).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&project.name)
                                    .size(14.5)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            if running {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let (rect, _) = ui.allocate_exact_size(
                                            egui::vec2(8.0, 8.0),
                                            egui::Sense::hover(),
                                        );
                                        ui.painter().circle_filled(
                                            rect.center(),
                                            4.0,
                                            theme::ACCENT,
                                        );
                                    },
                                );
                            }
                        });
                        ui.add_space(2.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(crate::ui::truncate(&project.root_path, 30))
                                    .size(11.0)
                                    .monospace()
                                    .color(theme::FAINT),
                            )
                            .truncate(),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{goals} objetivo{}",
                                    if goals == 1 { "" } else { "s" }
                                ))
                                .size(11.5)
                                .color(if running {
                                    theme::ACCENT
                                } else {
                                    theme::MUTED
                                }),
                            );
                            if project.auto_merge {
                                theme::pill(theme::SURFACE_RAISED).show(ui, |ui| {
                                    ui.label(
                                        RichText::new("auto-merge").size(10.5).color(theme::MUTED),
                                    );
                                });
                            }
                        });
                    });
                    if ui
                        .interact(
                            card.response.rect,
                            ui.id().with(("project-card", &project.id)),
                            egui::Sense::click(),
                        )
                        .clicked()
                    {
                        app.select_project(project.id.clone());
                    }
                    ui.add_space(8.0);
                }
            });
        });
}

fn details(context: &egui::Context, app: &mut Studio) {
    egui::SidePanel::right("details")
        .resizable(true)
        .default_width(400.0)
        .min_width(300.0)
        .max_width(560.0)
        .frame(
            egui::Frame::new()
                .fill(theme::SURFACE)
                .inner_margin(egui::Margin::symmetric(14, 10))
                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER)),
        )
        .show(context, |ui| {
            ui.label(
                RichText::new("Detalle")
                    .size(12.0)
                    .strong()
                    .color(theme::FAINT),
            );
            ui.add_space(6.0);
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

        if !show_projects {
            // Narrow window: the project list becomes a selector instead of a panel.
            let current = app
                .state
                .project()
                .map(|project| project.name.clone())
                .unwrap_or_else(|| "proyecto".to_string());
            egui::ComboBox::from_id_salt("project-selector")
                .selected_text(current)
                .width(220.0)
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
            ui.add_space(4.0);
        }

        workspace_nav(ui, app);
        ui.add_space(6.0);
        workspace_status(ui, app);
        ui.add_space(8.0);

        match app.tab {
            Tab::Orchestrator => orchestrator::panel(ui, app),
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

fn workspace_nav(ui: &mut egui::Ui, app: &mut Studio) {
    theme::bar().show(ui, |ui| {
        egui::ScrollArea::horizontal()
            .id_salt("workspace-nav-scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    ui.spacing_mut().button_padding = egui::vec2(9.0, 5.0);
                    nav_tab(ui, &mut app.tab, Tab::Orchestrator, "Orquestador");
                    nav_tab(ui, &mut app.tab, Tab::Goals, "Objetivos");
                    nav_tab(ui, &mut app.tab, Tab::Code, "Código");
                    if !app.wide {
                        nav_tab(ui, &mut app.tab, Tab::Details, "Detalle");
                    }
                    nav_tab(
                        ui,
                        &mut app.tab,
                        Tab::Extensions,
                        if app.wide { "Skills/MCP" } else { "Skills" },
                    );
                    nav_tab(ui, &mut app.tab, Tab::Policy, "Política");
                    nav_tab(ui, &mut app.tab, Tab::Memory, "Memoria");
                    nav_tab(ui, &mut app.tab, Tab::System, "Sistema");
                });
            });
    });
}

fn nav_tab(ui: &mut egui::Ui, current: &mut Tab, tab: Tab, label: &str) {
    let selected = *current == tab;
    let text = RichText::new(label).size(13.0).color(if selected {
        theme::ACCENT
    } else {
        theme::MUTED
    });
    if ui.selectable_label(selected, text).clicked() {
        *current = tab;
    }
}

fn workspace_status(ui: &mut egui::Ui, app: &Studio) {
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
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(7.0, 7.0), egui::Sense::hover());
        ui.painter().circle_filled(
            rect.center(),
            3.5,
            if working > 0 {
                theme::ACCENT
            } else {
                theme::FAINT
            },
        );
        ui.label(
            RichText::new(format!(
                "{active} objetivo{} activo{} · {working} tarea{} en curso",
                if active == 1 { "" } else { "s" },
                if active == 1 { "" } else { "s" },
                if working == 1 { "" } else { "s" },
            ))
            .size(11.5)
            .color(theme::MUTED),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(project) = app.state.project() {
                ui.label(
                    RichText::new(&project.base_branch)
                        .size(11.5)
                        .monospace()
                        .color(theme::FAINT),
                );
                ui.label(RichText::new("rama base").size(11.5).color(theme::FAINT));
            }
        });
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
        theme::DANGER
    } else {
        theme::SUCCESS
    };
    egui::Frame::new()
        .fill(color.gamma_multiply(0.11))
        .stroke(egui::Stroke::new(1.0_f32, color.gamma_multiply(0.45)))
        .corner_radius(theme::RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(if notice.error { "!" } else { "✓" })
                        .size(14.0)
                        .strong()
                        .color(color),
                );
                ui.label(
                    RichText::new(if notice.error { "Error" } else { "Hecho" })
                        .size(12.5)
                        .strong()
                        .color(color),
                );
                ui.label(RichText::new(&notice.message).size(13.0).color(theme::TEXT));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("×").size(14.0).color(theme::MUTED))
                                .min_size(egui::vec2(26.0, 26.0)),
                        )
                        .clicked()
                    {
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
        .default_height(150.0)
        .min_height(90.0)
        .max_height(380.0)
        .frame(
            egui::Frame::new()
                .fill(theme::SURFACE)
                .inner_margin(egui::Margin::symmetric(12, 6))
                .stroke(egui::Stroke::new(1.0_f32, theme::BORDER)),
        )
        .show(context, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                ui.spacing_mut().button_padding = egui::vec2(10.0, 5.0);
                console_tab(
                    ui,
                    &mut app.console_tab,
                    crate::app::ConsoleTab::Events,
                    "Eventos",
                );
                console_tab(
                    ui,
                    &mut app.console_tab,
                    crate::app::ConsoleTab::Agent,
                    "Chat",
                );
                let label = if app.terminals.is_empty() {
                    "Terminal".to_string()
                } else {
                    format!("Terminal · {}", app.terminals.len())
                };
                console_tab(
                    ui,
                    &mut app.console_tab,
                    crate::app::ConsoleTab::Terminals,
                    &label,
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("×").size(15.0).color(theme::MUTED))
                                .min_size(egui::vec2(28.0, 28.0)),
                        )
                        .on_hover_text("cerrar consola")
                        .clicked()
                    {
                        app.show_console = false;
                    }
                });
            });
            ui.add_space(3.0);
            ui.separator();
            ui.add_space(2.0);
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

fn console_tab(
    ui: &mut egui::Ui,
    current: &mut crate::app::ConsoleTab,
    tab: crate::app::ConsoleTab,
    label: &str,
) {
    let selected = *current == tab;
    let text = RichText::new(label).size(12.5).color(if selected {
        theme::ACCENT
    } else {
        theme::MUTED
    });
    if ui.selectable_label(selected, text).clicked() {
        *current = tab;
    }
}

fn events(ui: &mut egui::Ui, app: &mut Studio) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.event_filter)
                .hint_text("filtrar eventos")
                .desired_width(240.0),
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
        "succeeded" => theme::SUCCESS,
        "failed" | "timed_out" | "interrupted" => theme::DANGER,
        "blocked" => theme::WARNING,
        "running" | "verifying" | "started" => theme::ACCENT,
        "cancelled" => theme::FAINT,
        _ => theme::NEUTRAL,
    }
}

pub fn badge(ui: &mut egui::Ui, status: &str) {
    let color = status_color(status);
    theme::pill(color.gamma_multiply(0.14)).show(ui, |ui| {
        ui.label(RichText::new(status).size(11.0).color(color).strong());
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
                egui::vec2((segment - 2.0).max(2.0), 6.0),
                egui::Sense::hover(),
            );
            let fill = if index < progress.filled {
                color
            } else {
                theme::BORDER
            };
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(3), fill);
        }
        ui.label(
            RichText::new(progress.stage.label())
                .size(11.5)
                .color(color),
        );
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
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 9.0), egui::Sense::hover());
    for (index, task) in tasks.iter().enumerate() {
        let progress = crate::state::task_progress(task);
        let color = status_color(progress.stage.status_key());
        let piece = egui::Rect::from_min_size(
            egui::pos2(rect.left() + index as f32 * segment, rect.top()),
            egui::vec2((segment - 2.0).max(1.0), 9.0),
        );
        ui.painter()
            .rect_filled(piece, egui::CornerRadius::same(3), color);
    }
}

pub fn metric(ui: &mut egui::Ui, label: &str, value: impl Into<String>) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new(value.into())
                .strong()
                .size(17.0)
                .color(theme::TEXT),
        );
        ui.label(RichText::new(label).size(11.0).color(theme::FAINT));
    });
}
