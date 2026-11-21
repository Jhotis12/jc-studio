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
    iteration_window(context, app);
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
        .default_width(820.0)
        .min_width(640.0)
        .default_height(560.0)
        .min_height(380.0)
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

    let models = app.state.models.clone();
    egui::Grid::new("runtime-settings")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("Modelo por defecto");
            ui.horizontal(|ui| {
                crate::ui::model_selector(
                    ui,
                    "default-model",
                    &mut app.global_form.default_model,
                    &models,
                );
            });
            ui.end_row();

            ui.label("Agente por defecto");
            ui.add(
                egui::TextEdit::singleline(&mut app.global_form.default_agent)
                    .hint_text("el del backend")
                    .desired_width(280.0),
            );
            ui.end_row();

            ui.label("Modelo de planificación");
            ui.horizontal(|ui| {
                crate::ui::model_selector(
                    ui,
                    "planning-model",
                    &mut app.global_form.planning_model,
                    &models,
                );
            });
            ui.end_row();

            ui.label("Agente de planificación");
            ui.add(
                egui::TextEdit::singleline(&mut app.global_form.planning_agent)
                    .desired_width(280.0),
            );
            ui.end_row();

            ui.label("Modelo de revisión");
            ui.horizontal(|ui| {
                crate::ui::model_selector(
                    ui,
                    "review-model",
                    &mut app.global_form.review_model,
                    &models,
                );
            });
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

    ui.add_space(10.0);
    ui.separator();
    ui.label(RichText::new("Telegram").strong());
    let telegram = app.state.settings.as_ref().map(|settings| {
        (
            settings.telegram_configured,
            settings.telegram_token_hint.clone(),
            settings.telegram_chat_ids.len(),
        )
    });
    match &telegram {
        Some((true, hint, _)) => {
            let hint = hint.clone().unwrap_or_default();
            ui.label(
                RichText::new(format!("token configurado · …{hint}"))
                    .small()
                    .color(Color32::from_rgb(120, 200, 130)),
            );
        }
        Some((false, _, _)) => {
            ui.label(
                RichText::new("sin token: el bot está apagado")
                    .small()
                    .weak(),
            );
        }
        None => {
            ui.label(RichText::new("cargando…").small().weak());
        }
    }
    if telegram.is_some_and(|(_, _, chats)| chats == 0) {
        ui.label(
            RichText::new("sin chats permitidos: el bot no responderá a nadie")
                .small()
                .color(Color32::from_rgb(230, 170, 90)),
        );
    }

    ui.horizontal(|ui| {
        ui.label("Token");
        ui.add_enabled(
            !app.global_form.clear_telegram_token,
            egui::TextEdit::singleline(&mut app.global_form.telegram_token)
                .password(true)
                .hint_text("el que da @BotFather")
                .desired_width(280.0),
        );
    });
    ui.checkbox(
        &mut app.global_form.clear_telegram_token,
        "borrar el token guardado",
    );
    ui.horizontal(|ui| {
        ui.label("Chats permitidos");
        ui.add(
            egui::TextEdit::singleline(&mut app.global_form.telegram_chat_ids)
                .hint_text("123456789, -1001234567890")
                .desired_width(280.0),
        );
    });
    ui.label(
        RichText::new(
            "El token se guarda en secrets.json con permisos 0600, nunca en la base de \
             datos ni en la API. Los chats son los ids que pueden dar órdenes al bot. \
             Se aplica al reiniciar jcsd.",
        )
        .weak()
        .small(),
    );

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

    ui.add_space(10.0);
    ui.separator();
    ui.label(RichText::new("Aplicar cambios que requieren reinicio").strong());
    ui.label(
        RichText::new(
            "La concurrencia, el token de Telegram y los chats se leen al arrancar. Reiniciar \
             jcsd interrumpe las ejecuciones en curso: se marcan como interrumpidas y sus \
             tareas como bloqueadas, nunca se pierde el trabajo ya confirmado.",
        )
        .weak()
        .small(),
    );
    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.confirm_restart,
            "entiendo que se interrumpen las tareas en curso",
        );
        if ui
            .add_enabled(
                app.confirm_restart && app.state.is_connected(),
                egui::Button::new("Reiniciar jcsd"),
            )
            .clicked()
        {
            match restart_daemon() {
                Ok(message) => {
                    app.confirm_restart = false;
                    app.state.notice = Some(crate::state::Notice {
                        message,
                        error: false,
                    });
                    app.daemon.send(crate::daemon::Command::Refresh);
                }
                Err(message) => {
                    app.state.notice = Some(crate::state::Notice {
                        message,
                        error: true,
                    });
                }
            }
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

/// Restarts the user service. The application reconnects on its own; work in flight is
/// recovered as interrupted/blocked by the daemon's startup path.
fn restart_daemon() -> Result<String, String> {
    match std::process::Command::new("systemctl")
        .args(["--user", "restart", "jcsd"])
        .status()
    {
        Ok(status) if status.success() => {
            Ok("jcsd reiniciado; la app se reconectará en unos segundos".to_string())
        }
        Ok(status) => Err(format!(
            "systemctl no pudo reiniciar jcsd (código {status})"
        )),
        Err(error) => Err(format!(
            "no se pudo ejecutar systemctl: {error}. Reinicia el servicio a mano."
        )),
    }
}

/// Feedback for a task. Succeeded tasks get a dependent follow-up; failed/blocked tasks are
/// retried with the new instructions.
fn iteration_window(context: &egui::Context, app: &mut Studio) {
    let Some(task_id) = app.iteration_for.clone() else {
        return;
    };
    let Some(task) = app.state.task(&task_id).cloned() else {
        app.iteration_for = None;
        return;
    };

    let mut open = true;
    let mut feedback = std::mem::take(&mut app.iteration_feedback);
    let mut send = false;
    egui::Window::new("Pedir cambios")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .show(context, |ui| {
            ui.horizontal_wrapped(|ui| {
                crate::ui::badge(ui, &task.status);
                ui.label(RichText::new(&task.title).strong());
            });
            ui.label(
                RichText::new(match task.status.as_str() {
                    "succeeded" => {
                        "Se creará una tarea nueva que depende de esta y trabajará en su                          worktree/integración. El resultado original no se sobrescribe."
                    }
                    "failed" | "blocked" => {
                        "La tarea vuelve a la cola con estas indicaciones; el worktree se                          reutiliza."
                    }
                    _ => "Las indicaciones se añadirán antes de que la tarea se ejecute.",
                })
                .weak()
                .small(),
            );
            if let Some(instructions) = &task.instructions {
                if !instructions.trim().is_empty() {
                    ui.collapsing("Instrucciones actuales", |ui| {
                        ui.label(RichText::new(instructions).monospace().small().weak());
                    });
                }
            }
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::multiline(&mut feedback)
                    .desired_rows(5)
                    .desired_width(f32::INFINITY)
                    .hint_text(
                        "Qué debe cambiar, qué falta o qué hizo mal. Sé concreto: el agente \
                         solo verá este texto y el contexto de la tarea.",
                    ),
            );
            ui.add_space(6.0);
            let can_send = !feedback.trim().is_empty()
                || matches!(task.status.as_str(), "failed" | "blocked" | "pending");
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(can_send, egui::Button::new("Enviar"))
                    .clicked()
                {
                    send = true;
                }
                if ui.button("Cancelar").clicked() {
                    send = false;
                    // The window closes through `open` below.
                }
            });
        });

    if send {
        app.daemon.send(crate::daemon::Command::IterateTask {
            task: task_id,
            feedback,
        });
        app.iteration_for = None;
    } else {
        app.iteration_feedback = feedback;
    }
    if !open {
        app.iteration_for = None;
    }
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
