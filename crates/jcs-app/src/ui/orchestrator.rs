//! Project-level conversation with the orchestrator.
//!
//! Distinct from a goal: this chat answers questions and proposes work; it never creates
//! tasks by itself. Execution stays in the objectives board.

use egui::RichText;

use crate::app::Studio;
use crate::daemon::Command;
use crate::ui::theme;

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

    // Reserve the composer before painting the transcript. A bottom panel created after
    // the scroll area can overlap it in short windows.
    egui::TopBottomPanel::bottom("orchestrator-compose")
        .resizable(false)
        .exact_height(112.0)
        .show_inside(ui, |ui| {
            ui.add_space(4.0);
            ui.separator();
            let editor = ui.add(
                egui::TextEdit::multiline(&mut app.orchestrator_input)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Body)
                    .hint_text("Escribe al orquestador…"),
            );
            let submit =
                ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                let can_send =
                    !app.state.orchestrator_busy && !app.orchestrator_input.trim().is_empty();
                if ui
                    .add_enabled(
                        can_send,
                        egui::Button::new(
                            RichText::new("Enviar").size(12.5).strong().color(theme::BG),
                        )
                        .fill(theme::ACCENT)
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(egui::CornerRadius::same(7))
                        .min_size(egui::vec2(84.0, 30.0)),
                    )
                    .on_hover_text("Ctrl+Enter")
                    .clicked()
                {
                    app.send_orchestrator();
                }
                if app.state.orchestrator_busy {
                    ui.spinner();
                    ui.label(
                        RichText::new("esperando respuesta…")
                            .size(11.0)
                            .color(theme::MUTED),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(&project.name).size(11.0).color(theme::FAINT));
                });
            });
            if submit && editor.has_focus() {
                app.send_orchestrator();
            }
        });

    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Orquestador")
                .size(16.0)
                .strong()
                .color(theme::TEXT),
        );
        ui.label(
            RichText::new("conversa, explora el proyecto y pide un plan")
                .size(11.5)
                .color(theme::MUTED),
        );
        ui.add(
            egui::Button::new(RichText::new("?").size(12.0).color(theme::MUTED))
                .min_size(egui::vec2(24.0, 24.0)),
        )
        .on_hover_text(
            "Este chat no ejecuta trabajo ni crea tareas. Cuando tengas claro el cambio, \
             pulsa «Crear objetivo con esta respuesta» y apruébalo. El consumo del chat \
             todavía no se suma al total de runs.",
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(
                    !app.state.orchestrator_busy,
                    egui::Button::new(RichText::new("Nueva conversación").size(11.5)),
                )
                .on_hover_text("Borra el historial y empieza una sesión limpia.")
                .clicked()
            {
                app.daemon.send(Command::ResetOrchestrator {
                    project: project.name.clone(),
                });
            }
            if ui
                .add(egui::Button::new(RichText::new("Actualizar").size(11.5)))
                .clicked()
            {
                app.daemon.send(Command::LoadOrchestrator {
                    project: project.name.clone(),
                });
            }
        });
    });
    ui.add_space(4.0);
    ui.separator();

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
                    .size(13.0)
                    .color(theme::MUTED),
                );
            }
            for message in &messages {
                bubble(ui, app, message);
            }
            if !app.state.orchestrator_reasoning.is_empty() {
                egui::CollapsingHeader::new(
                    RichText::new("Pensamiento en vivo")
                        .size(11.0)
                        .color(theme::FAINT),
                )
                .default_open(true)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(crate::ui::truncate(
                            app.state.orchestrator_reasoning.trim(),
                            4_000,
                        ))
                        .italics()
                        .size(12.5)
                        .color(theme::MUTED),
                    );
                });
            }
            if !app.state.orchestrator_streaming.is_empty() {
                egui::Frame::new()
                    .fill(theme::SURFACE_RAISED)
                    .corner_radius(theme::RADIUS_MD)
                    .inner_margin(egui::Margin::symmetric(11, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Orquestador")
                                    .size(11.0)
                                    .strong()
                                    .color(theme::MUTED),
                            );
                            ui.label(RichText::new("escribiendo…").size(11.0).color(theme::FAINT));
                        });
                        crate::ui::markdown::render(ui, &app.state.orchestrator_streaming);
                    });
                ui.add_space(3.0);
            } else if app.state.orchestrator_busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        RichText::new("El orquestador está pensando…")
                            .size(12.0)
                            .color(theme::MUTED),
                    );
                });
            }
        });
}

fn bubble(ui: &mut egui::Ui, app: &mut Studio, message: &jcs_proto::OrchestratorMessage) {
    let user = message.role == "user";
    egui::Frame::new()
        .fill(if user {
            theme::ACCENT_SOFT.gamma_multiply(0.55)
        } else {
            theme::SURFACE_RAISED
        })
        .corner_radius(theme::RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(11, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if user { "Tú" } else { "Orquestador" })
                        .size(11.0)
                        .strong()
                        .color(if user { theme::ACCENT } else { theme::MUTED }),
                );
                ui.label(
                    RichText::new(message.created_at.strftime("%H:%M").to_string())
                        .size(10.5)
                        .color(theme::FAINT),
                );
            });
            crate::ui::markdown::render(ui, &message.text);
            if !user
                && ui
                    .add(
                        egui::Button::new(
                            RichText::new("Crear objetivo con esta respuesta")
                                .size(11.5)
                                .color(theme::ACCENT),
                        )
                        .fill(theme::ACCENT_SOFT)
                        .stroke(egui::Stroke::new(
                            1.0_f32,
                            theme::ACCENT.gamma_multiply(0.45),
                        ))
                        .corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(0.0, 28.0)),
                    )
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
