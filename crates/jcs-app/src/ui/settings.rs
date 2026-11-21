//! Modal windows: project settings, project registration and confirmation.
//!
//! Forms are explicit: validation happens in the daemon, and the window only sends values.
//! Nothing here writes SQLite, git or policy files directly.

use egui::{Color32, RichText};

use crate::app::{parse_amount, parse_tokens, Studio};
use crate::daemon::Command;

pub fn windows(context: &egui::Context, app: &mut Studio) {
    settings_window(context, app);
    add_project_window(context, app);
}

fn settings_window(context: &egui::Context, app: &mut Studio) {
    if !app.show_settings {
        return;
    }
    app.sync_project_form();
    app.sync_global_form();

    let mut open = app.show_settings;
    egui::Window::new("Ajustes")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .show(context, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                project_settings(ui, app);
                ui.add_space(12.0);
                ui.separator();
                daemon_settings(ui, app);
            });
        });
    app.show_settings = open;
}

fn project_settings(ui: &mut egui::Ui, app: &mut Studio) {
    ui.label(RichText::new("Proyecto").strong());
    let Some(project) = app.state.project().cloned() else {
        ui.label(RichText::new("Selecciona un proyecto.").weak());
        return;
    };
    ui.label(RichText::new(&project.name).monospace().small());
    ui.label(RichText::new(&project.root_path).weak().small());
    ui.add_space(6.0);

    ui.checkbox(&mut app.project_form.auto_merge, "Integrar automáticamente")
        .on_hover_text(
            "Si los gates pasan y el cambio no supera el techo de riesgo, la rama del objetivo \
             se integra en la base. Desactivado, el trabajo queda en la rama del objetivo para \
             que lo revises.",
        );

    ui.add_space(6.0);
    ui.label(RichText::new("Presupuesto diario").strong());
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.project_form.budget_usd)
                .hint_text("sin límite")
                .desired_width(110.0),
        );
        ui.label("USD");
        ui.add_space(12.0);
        ui.add(
            egui::TextEdit::singleline(&mut app.project_form.budget_tokens)
                .hint_text("sin límite")
                .desired_width(130.0),
        );
        ui.label("tokens");
    });
    ui.label(
        RichText::new(
            "El techo bloquea trabajo nuevo; nunca interrumpe un objetivo ya terminado. Deja un \
             campo vacío para no poner límite.",
        )
        .weak()
        .small(),
    );

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button("Guardar cambios").clicked() {
            // Validate locally only to show the error next to the field; the daemon is still
            // the authority and re-validates.
            if parse_amount(&app.project_form.budget_usd).is_err()
                || parse_tokens(&app.project_form.budget_tokens).is_err()
            {
                app.state.notice = Some(crate::state::Notice {
                    message: "presupuesto inválido".to_string(),
                    error: true,
                });
            } else {
                app.save_project_form();
            }
        }
        if ui.button("Descartar").clicked() {
            app.project_form.project = None;
            app.sync_project_form();
        }
    });

    ui.add_space(12.0);
    ui.separator();
    egui::CollapsingHeader::new("Zona peligrosa")
        .default_open(false)
        .show(ui, |ui| {
            ui.label(
                RichText::new(
                    "Eliminar el proyecto no borra el repositorio ni sus commits; deja de \
                     gestionarlo y sus objetivos salen de la lista.",
                )
                .weak()
                .small(),
            );
            ui.checkbox(
                &mut app.confirm_remove,
                "Confirmo que quiero dejar de gestionar este proyecto",
            );
            if ui
                .add_enabled(app.confirm_remove, egui::Button::new("Eliminar proyecto"))
                .clicked()
            {
                app.daemon.send(Command::RemoveProject {
                    id: project.id.clone(),
                });
                app.confirm_remove = false;
                app.show_settings = false;
            }
        });
}

fn daemon_settings(ui: &mut egui::Ui, app: &mut Studio) {
    ui.label(RichText::new("Daemon").strong());
    match &app.state.health {
        Some(health) => {
            egui::Grid::new("daemon-settings")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("socket").weak());
                    ui.label(
                        RichText::new(jcs_proto::paths::socket_path().display().to_string())
                            .monospace()
                            .small(),
                    );
                    ui.end_row();
                    ui.label(RichText::new("base de datos").weak());
                    ui.label(RichText::new(&health.database).monospace().small());
                    ui.end_row();
                    ui.label(RichText::new("versión").weak());
                    ui.label(format!(
                        "{} · protocolo {}",
                        health.version, health.protocol
                    ));
                    ui.end_row();
                    ui.label(RichText::new("uptime").weak());
                    ui.label(format!("{} s", health.uptime_secs));
                    ui.end_row();
                    ui.label(RichText::new("agente").weak());
                    ui.label(if health.agent.reachable {
                        "disponible"
                    } else {
                        "no disponible"
                    });
                    ui.end_row();
                });
        }
        None => {
            ui.colored_label(
                Color32::from_rgb(225, 110, 110),
                "El daemon no está conectado. Arráncalo con `systemctl --user start jcsd`.",
            );
        }
    }

    ui.add_space(10.0);
    ui.label(RichText::new("Modelos y ejecución").strong());
    ui.label(
        RichText::new(
            "Estos valores se guardan en la base y se aplican al siguiente despacho. Un campo              vacío significa «lo que decida el backend».",
        )
        .weak()
        .small(),
    );
    ui.add_space(4.0);

    egui::Grid::new("runtime-settings")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("Modelo por defecto");
            ui.add(
                egui::TextEdit::singleline(&mut app.global_form.default_model)
                    .hint_text("provider/modelo")
                    .desired_width(280.0),
            );
            ui.end_row();

            ui.label("Agente por defecto");
            ui.add(
                egui::TextEdit::singleline(&mut app.global_form.default_agent)
                    .hint_text("el del backend")
                    .desired_width(280.0),
            );
            ui.end_row();

            ui.label("Modelo de planificación");
            ui.add(
                egui::TextEdit::singleline(&mut app.global_form.planning_model)
                    .hint_text("hereda el modelo por defecto")
                    .desired_width(280.0),
            );
            ui.end_row();

            ui.label("Agente de planificación");
            ui.add(
                egui::TextEdit::singleline(&mut app.global_form.planning_agent)
                    .desired_width(280.0),
            );
            ui.end_row();

            ui.label("Modelo de revisión");
            ui.add(
                egui::TextEdit::singleline(&mut app.global_form.review_model)
                    .hint_text("hereda el modelo por defecto")
                    .desired_width(280.0),
            );
            ui.end_row();

            ui.label("Revisión con rúbrica");
            ui.checkbox(
                &mut app.global_form.review,
                "revisar antes de integrar (duplica el coste)",
            );
            ui.end_row();

            ui.label("Techo de auto-merge (líneas)");
            ui.add(
                egui::TextEdit::singleline(&mut app.global_form.risk_max_lines)
                    .desired_width(100.0),
            );
            ui.end_row();

            ui.label("Timeout por ejecución (s)");
            ui.add(
                egui::TextEdit::singleline(&mut app.global_form.run_timeout_secs)
                    .desired_width(100.0),
            );
            ui.end_row();

            ui.label("Contexto inyectado (tokens)");
            ui.add(
                egui::TextEdit::singleline(&mut app.global_form.context_tokens)
                    .desired_width(100.0),
            );
            ui.end_row();

            ui.label("Ciclos de reparación");
            ui.add(
                egui::TextEdit::singleline(&mut app.global_form.max_repair_cycles)
                    .desired_width(100.0),
            );
            ui.end_row();

            ui.label("Concurrencia máxima");
            ui.add(
                egui::TextEdit::singleline(&mut app.global_form.max_concurrent)
                    .desired_width(100.0),
            );
            ui.end_row();
        });

    if let Some(settings) = &app.state.settings {
        if !settings.restart_required.is_empty() {
            egui::Frame::group(ui.style())
                .fill(Color32::from_rgb(230, 170, 90).gamma_multiply(0.12))
                .show(ui, |ui| {
                    ui.colored_label(
                        Color32::from_rgb(235, 190, 110),
                        format!(
                            "Requiere reiniciar jcsd: {}",
                            settings.restart_required.join(", ")
                        ),
                    );
                    ui.label(
                        RichText::new("systemctl --user restart jcsd")
                            .monospace()
                            .small()
                            .weak(),
                    );
                });
        }
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                app.state.is_connected(),
                egui::Button::new("Guardar ajustes del daemon"),
            )
            .clicked()
        {
            app.save_global_settings();
        }
        if ui.button("Recargar").clicked() {
            app.settings_synced = false;
            app.daemon.send(crate::daemon::Command::LoadSettings);
        }
    });
    ui.label(
        RichText::new(
            "El token de Telegram y las claves de embeddings no se editan aquí: son secretos y              pertenecen al llavero del sistema, no a la base de datos.",
        )
        .weak()
        .small(),
    );
}

fn add_project_window(context: &egui::Context, app: &mut Studio) {
    if !app.show_add_project {
        return;
    }
    let mut open = app.show_add_project;
    egui::Window::new("Añadir proyecto")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(480.0)
        .show(context, |ui| {
            ui.label("Ruta del repositorio");
            ui.add(
                egui::TextEdit::singleline(&mut app.add_project.path)
                    .hint_text("/home/tú/código/mi-repo")
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(6.0);
            ui.label("Nombre (opcional)");
            ui.add(
                egui::TextEdit::singleline(&mut app.add_project.name)
                    .hint_text("se deduce del directorio")
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(6.0);
            ui.label("Rama base");
            ui.add(
                egui::TextEdit::singleline(&mut app.add_project.base_branch)
                    .hint_text("main")
                    .desired_width(180.0),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "La ruta debe ser un repositorio git. El daemon no modifica la rama base \
                     sin auto-merge, árbol limpio y fast-forward.",
                )
                .weak()
                .small(),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let path = app.add_project.path.trim().to_string();
                if ui
                    .add_enabled(!path.is_empty(), egui::Button::new("Añadir proyecto"))
                    .clicked()
                {
                    let name = app.add_project.name.trim();
                    let branch = app.add_project.base_branch.trim();
                    app.daemon.send(Command::AddProject {
                        path,
                        name: (!name.is_empty()).then(|| name.to_string()),
                        base_branch: (!branch.is_empty()).then(|| branch.to_string()),
                    });
                    app.add_project = Default::default();
                    app.show_add_project = false;
                }
                if ui.button("Cancelar").clicked() {
                    app.show_add_project = false;
                }
            });
        });
    app.show_add_project = open;
}
