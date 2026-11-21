//! Skills and MCP: what the agents can load, not just models.

use egui::{Color32, RichText};

use crate::app::{ExtensionsTab, McpKind, Studio};
use crate::daemon::Command;
use crate::ui::badge;

pub fn panel(ui: &mut egui::Ui, app: &mut Studio) {
    let Some(project) = app.state.project().cloned() else {
        ui.label("Selecciona un proyecto.");
        return;
    };

    if app.extensions_loaded_for.as_deref() != Some(project.id.as_str()) {
        app.extensions_loaded_for = Some(project.id.clone());
        app.daemon.send(Command::LoadSkills {
            project: project.name.clone(),
        });
        app.daemon.send(Command::LoadMcp {
            project: project.name.clone(),
        });
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Skills y MCP").strong());
        ui.label(
            RichText::new("lo que el agente puede cargar además del modelo")
                .weak()
                .small(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Actualizar").clicked() {
                app.daemon.send(Command::LoadSkills {
                    project: project.name.clone(),
                });
                app.daemon.send(Command::LoadMcp {
                    project: project.name.clone(),
                });
            }
        });
    });
    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut app.extensions_tab,
            ExtensionsTab::Skills,
            format!("Skills · {}", app.state.skills.len()),
        );
        ui.selectable_value(
            &mut app.extensions_tab,
            ExtensionsTab::Mcp,
            format!("MCP · {}", app.state.mcp.len()),
        );
    });
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| match app.extensions_tab {
            ExtensionsTab::Skills => skills(ui, app),
            ExtensionsTab::Mcp => mcp(ui, app, &project.name),
        });
}

fn skills(ui: &mut egui::Ui, app: &mut Studio) {
    ui.label(RichText::new("Skills").strong());
    ui.label(
        RichText::new(
            "Las marcadas se activan en la sesión de cada tarea nueva. No cambian las tareas \
             que ya están en marcha.",
        )
        .weak()
        .small(),
    );
    ui.add_space(4.0);

    let selected = app
        .state
        .settings
        .as_ref()
        .map(|settings| settings.agent_skills.clone())
        .unwrap_or_default();
    let can_edit = app.state.settings.is_some();

    ui.add(
        egui::TextEdit::singleline(&mut app.skill_filter)
            .hint_text("filtrar skills por nombre o descripción")
            .desired_width(f32::INFINITY),
    );
    let filter = app.skill_filter.to_lowercase();
    let visible: Vec<jcs_proto::SkillCatalogEntry> = app
        .state
        .skills
        .iter()
        .filter(|skill| {
            filter.is_empty()
                || skill.id.to_lowercase().contains(&filter)
                || skill.name.to_lowercase().contains(&filter)
                || skill.description.to_lowercase().contains(&filter)
        })
        .cloned()
        .collect();
    if !filter.is_empty() {
        ui.label(
            RichText::new(format!(
                "{} de {} skills",
                visible.len(),
                app.state.skills.len()
            ))
            .weak()
            .small(),
        );
    }
    ui.add_space(4.0);

    if app.state.skills.is_empty() {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label("OpenCode no tiene skills registradas para este proyecto.");
            ui.label(
                RichText::new(
                    "Instálalas como markdown en ~/.config/opencode/skills (o en la carpeta de \
                     skills de OpenCode), reinicia OpenCode y pulsa Actualizar.",
                )
                .weak()
                .small(),
            );
        });
        return;
    }

    for skill in visible {
        let mut enabled = selected.contains(&skill.id);
        let response = ui.add_enabled(
            can_edit,
            egui::Checkbox::new(
                &mut enabled,
                RichText::new(format!(
                    "{}  ·  {}{}",
                    skill.name,
                    skill.id,
                    if skill.autoinvoke { "  ·  auto" } else { "" }
                ))
                .strong(),
            ),
        );
        if response.changed() {
            app.toggle_skill(skill.id.clone(), enabled);
        }
        if !skill.description.is_empty() {
            ui.label(RichText::new(&skill.description).small().weak());
        }
        ui.label(RichText::new(&skill.path).monospace().weak().small());
        ui.add_space(4.0);
    }
}

fn mcp(ui: &mut egui::Ui, app: &mut Studio, project: &str) {
    ui.label(RichText::new("Servidores MCP").strong());
    ui.label(
        RichText::new(
            "Conectar, desconectar y quitar actúa sobre el OpenCode en ejecución. Para que un \
             servidor sobreviva al reinicio de OpenCode, añádelo también a su configuración.",
        )
        .weak()
        .small(),
    );
    ui.add_space(4.0);

    if app.state.mcp.is_empty() {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label("No hay servidores MCP configurados en OpenCode.");
        });
    }

    for server in app.state.mcp.clone() {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                badge(
                    ui,
                    match server.status.as_str() {
                        "connected" => "conectado",
                        "pending" => "pendiente",
                        "needs_auth" => "necesita auth",
                        "failed" => "falló",
                        _ => "desconectado",
                    },
                );
                ui.label(RichText::new(&server.name).monospace().strong());
                if let Some(title) = &server.title {
                    ui.label(RichText::new(title).small().weak());
                }
            });
            if let Some(error) = &server.error {
                ui.label(
                    RichText::new(error)
                        .small()
                        .color(Color32::from_rgb(225, 110, 110)),
                );
            }
            ui.horizontal_wrapped(|ui| {
                if server.status == "connected" {
                    if ui.small_button("Desconectar").clicked() {
                        app.daemon.send(Command::DisconnectMcp {
                            project: project.to_string(),
                            server: server.name.clone(),
                        });
                    }
                } else if ui.small_button("Conectar").clicked() {
                    app.daemon.send(Command::ConnectMcp {
                        project: project.to_string(),
                        server: server.name.clone(),
                    });
                }
                if ui
                    .small_button("Quitar")
                    .on_hover_text("Lo quita del OpenCode en ejecución.")
                    .clicked()
                {
                    app.daemon.send(Command::RemoveMcp {
                        project: project.to_string(),
                        server: server.name.clone(),
                    });
                }
            });
        });
        ui.add_space(4.0);
    }

    ui.add_space(8.0);
    egui::CollapsingHeader::new("Añadir un servidor MCP")
        .default_open(false)
        .show(ui, |ui| {
            egui::Grid::new("mcp-add")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Nombre");
                    ui.add(
                        egui::TextEdit::singleline(&mut app.mcp_form.name)
                            .hint_text("p. ej. playwright")
                            .desired_width(240.0),
                    );
                    ui.end_row();

                    ui.label("Tipo");
                    egui::ComboBox::from_id_salt("mcp-kind")
                        .selected_text(match app.mcp_form.kind {
                            McpKind::Local => "local (comando)",
                            McpKind::Remote => "remoto (URL)",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.mcp_form.kind, McpKind::Local, "local");
                            ui.selectable_value(&mut app.mcp_form.kind, McpKind::Remote, "remoto");
                        });
                    ui.end_row();

                    match app.mcp_form.kind {
                        McpKind::Local => {
                            ui.label("Comando");
                            ui.add(
                                egui::TextEdit::singleline(&mut app.mcp_form.command)
                                    .hint_text("npx -y @paquete/servidor")
                                    .desired_width(320.0),
                            );
                        }
                        McpKind::Remote => {
                            ui.label("URL");
                            ui.add(
                                egui::TextEdit::singleline(&mut app.mcp_form.url)
                                    .hint_text("https://…")
                                    .desired_width(320.0),
                            );
                        }
                    }
                    ui.end_row();
                });
            if ui.button("Añadir MCP").clicked() {
                app.add_mcp();
            }
            ui.label(
                RichText::new(
                    "Los argumentos se separan por espacios. Un MCP local ejecuta un comando: \
                     asegúrate de confiar en él.",
                )
                .weak()
                .small(),
            );
        });
}
