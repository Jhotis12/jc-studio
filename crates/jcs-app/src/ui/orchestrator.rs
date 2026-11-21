//! Project-level conversation with the orchestrator.
//!
//! Distinct from a goal: this chat answers questions and proposes work; it never creates
//! tasks by itself. Execution stays in the objectives board.

use egui::{Color32, RichText};

use crate::app::Studio;
use crate::daemon::Command;

pub fn panel(ui: &mut egui::Ui, app: &mut Studio) {
    let Some(project) = app.state.project().cloned() else {
        ui.label("Selecciona un proyecto.");
        return;
    };

    if app.orchestrator_loaded_for.as_deref() != Some(project.id.as_str()) {
        app.orchestrator_loaded_for = Some(project.id.clone());
        app.daemon.send(Command::LoadOrchestrator {
            project: project.name.clone(),
        });
    }

    ui.horizontal(|ui| {
        ui.label(RichText::new("Orquestador").strong());
        ui.label(
            RichText::new("conversa, explora el proyecto y pide un plan")
                .weak()
                .small(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(
                    !app.state.orchestrator_busy,
                    egui::Button::new("Nueva conversación"),
                )
                .on_hover_text("Borra el historial y empieza una sesión limpia.")
                .clicked()
            {
                app.daemon.send(Command::ResetOrchestrator {
                    project: project.name.clone(),
                });
            }
            if ui.small_button("Actualizar").clicked() {
                app.daemon.send(Command::LoadOrchestrator {
                    project: project.name.clone(),
                });
            }
        });
    });
    ui.separator();

    ui.label(
        RichText::new(
            "Este chat no ejecuta trabajo. Cuando tengas claro el cambio, describe un \
             objetivo en «Objetivos»; el planificador lo convertirá en tareas. El consumo \
             de este chat todavía no se suma al total de runs.",
        )
        .weak()
        .small(),
    );
    ui.add_space(4.0);

    egui::TopBottomPanel::bottom("orchestrator-compose")
        .resizable(false)
        .exact_height(112.0)
        .show_inside(ui, |ui| {
            ui.separator();
            let editor = ui.add(
                egui::TextEdit::multiline(&mut app.orchestrator_input)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .hint_text("Escribe al orquestador…"),
            );
            let submit =
                ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                let can_send =
                    !app.state.orchestrator_busy && !app.orchestrator_input.trim().is_empty();
                if ui
                    .add_enabled(can_send, egui::Button::new("Enviar"))
                    .on_hover_text("Ctrl+Enter")
                    .clicked()
                {
                    app.send_orchestrator();
                }
                if app.state.orchestrator_busy {
                    ui.spinner();
                    ui.label(RichText::new("esperando respuesta…").weak().small());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(&project.name).weak().small());
                });
            });
            if submit && editor.has_focus() {
                app.send_orchestrator();
            }
        });

    let messages = app.state.orchestrator.clone();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(app.state.orchestrator_busy)
        .show(ui, |ui| {
            if messages.is_empty() {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(
                        "Pregúntale por la arquitectura, pídele que revise algo del repositorio \
                         o que proponga un plan. No se crearán tareas desde aquí.",
                    )
                    .weak(),
                );
            }
            for message in &messages {
                bubble(ui, app, message);
            }
            if !app.state.orchestrator_reasoning.is_empty() {
                egui::CollapsingHeader::new(
                    RichText::new("Pensamiento en vivo")
                        .small()
                        .color(Color32::from_rgb(130, 140, 165)),
                )
                .default_open(true)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(crate::ui::truncate(
                            app.state.orchestrator_reasoning.trim(),
                            4_000,
                        ))
                        .italics()
                        .small()
                        .color(Color32::from_rgb(130, 140, 165)),
                    );
                });
            }
            if !app.state.orchestrator_streaming.is_empty() {
                egui::Frame::new()
                    .fill(Color32::from_rgb(28, 32, 40))
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Orquestador")
                                    .small()
                                    .strong()
                                    .color(Color32::from_rgb(150, 190, 230)),
                            );
                            ui.label(RichText::new("escribiendo…").weak().small());
                        });
                        crate::ui::markdown::render(ui, &app.state.orchestrator_streaming);
                    });
                ui.add_space(3.0);
            } else if app.state.orchestrator_busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("El orquestador está pensando…".to_string());
                });
            }
        });
}

fn bubble(ui: &mut egui::Ui, app: &mut Studio, message: &jcs_proto::OrchestratorMessage) {
    let user = message.role == "user";
    let color = if user {
        Color32::from_rgb(48, 84, 130)
    } else {
        Color32::from_rgb(28, 32, 40)
    };
    egui::Frame::new()
        .fill(color.gamma_multiply(if user { 0.35 } else { 1.0 }))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if user { "Tú" } else { "Orquestador" })
                        .small()
                        .strong()
                        .color(if user {
                            Color32::from_rgb(140, 190, 245)
                        } else {
                            Color32::from_rgb(150, 190, 230)
                        }),
                );
                ui.label(
                    RichText::new(message.created_at.strftime("%H:%M").to_string())
                        .weak()
                        .small(),
                );
            });
            crate::ui::markdown::render(ui, &message.text);
            if !user
                && ui
                    .small_button("Crear objetivo con esta respuesta")
                    .clicked()
            {
                app.orchestrator_goal = Some(message.text.clone());
                app.orchestrator_goal_plan = true;
            }
        });
    ui.add_space(3.0);
}

/// Review and approve an objective proposed in the chat. Nothing is queued until the
/// operator presses the button: the same confirmation that stops a greeting from spending.
pub fn windows(context: &egui::Context, app: &mut Studio) {
    let Some(text) = app.orchestrator_goal.clone() else {
        return;
    };
    let Some(project) = app.state.project().map(|project| project.name.clone()) else {
        app.orchestrator_goal = None;
        return;
    };

    let mut open = true;
    let mut prompt = text;
    let mut plan = app.orchestrator_goal_plan;
    let mut create = false;
    let mut cancel = false;
    egui::Window::new("Crear objetivo desde el Orquestador")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(600.0)
        .show(context, |ui| {
            ui.label(
                RichText::new(
                    "Revisa y edita el objetivo. El orquestador no ejecuta nada: al crearlo \
                     pasa al tablero de Objetivos y el planificador lo descompone si está \
                     marcado.",
                )
                .weak()
                .small(),
            );
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::multiline(&mut prompt)
                    .desired_rows(5)
                    .desired_width(f32::INFINITY),
            );
            ui.checkbox(&mut plan, "Planificar en tareas");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !prompt.trim().is_empty(),
                        egui::Button::new("Crear objetivo"),
                    )
                    .clicked()
                {
                    create = true;
                }
                if ui.button("Cancelar").clicked() {
                    cancel = true;
                }
            });
        });

    if cancel {
        open = false;
    }
    if create {
        app.daemon.send(crate::daemon::Command::Ask {
            project,
            prompt: prompt.trim().to_string(),
            model: None,
            plan,
        });
        app.orchestrator_goal = None;
        app.tab = crate::app::Tab::Goals;
    } else if open {
        app.orchestrator_goal = Some(prompt);
        app.orchestrator_goal_plan = plan;
    } else {
        app.orchestrator_goal = None;
    }
}
