//! The objectives board: compose a goal, then watch its DAG move.
//!
//! The composer is the only way to start work; there is no command to remember. Execution
//! settings stay in the Settings window, so writing an objective is one text box.

use egui::{Color32, RichText};
use jcs_proto::GoalView;

use crate::app::Studio;
use crate::state::task_progress;
use crate::ui::{badge, status_color, theme};

pub fn goals(ui: &mut egui::Ui, app: &mut Studio) {
    let filter = app.goal_filter.to_lowercase();
    let goals: Vec<GoalView> = app
        .state
        .goals()
        .into_iter()
        .filter(|goal| {
            (app.show_finished_goals
                || !matches!(goal.status.as_str(), "succeeded" | "failed" | "cancelled"))
                && (filter.is_empty() || goal.prompt.to_lowercase().contains(&filter))
        })
        .cloned()
        .collect();

    // With work to watch, the list is the priority; the composer starts collapsed. The
    // operator can open it with one click and the choice is remembered.
    if !app.composer_initialized && app.state.project().is_some() {
        app.show_composer = goals.is_empty();
        app.composer_initialized = true;
    }
    if app.show_composer {
        composer(ui, app);
        ui.add_space(6.0);
        if goals.is_empty() {
            ui.add_space(26.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Todavía no hay objetivos")
                        .size(15.0)
                        .strong()
                        .color(theme::MUTED),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Cuando encoles uno, aquí verás su grafo, el progreso por tarea y la \
                         actividad de cada agente.",
                    )
                    .size(12.0)
                    .color(theme::FAINT),
                );
            });
            return;
        }
    }

    ui.horizontal_wrapped(|ui| {
        if app.show_composer {
            if ui
                .add(egui::Button::new(
                    RichText::new("Ocultar compositor")
                        .size(11.5)
                        .color(theme::MUTED),
                ))
                .clicked()
            {
                app.show_composer = false;
            }
        } else if ui
            .add(
                egui::Button::new(
                    RichText::new("+ Nuevo objetivo")
                        .size(12.0)
                        .strong()
                        .color(theme::BG),
                )
                .fill(theme::ACCENT)
                .stroke(egui::Stroke::NONE)
                .corner_radius(egui::CornerRadius::same(6))
                .min_size(egui::vec2(0.0, 29.0)),
            )
            .clicked()
        {
            app.show_composer = true;
        }
        ui.add(
            egui::TextEdit::singleline(&mut app.goal_filter)
                .hint_text("filtrar objetivos")
                .desired_width(200.0),
        );
        ui.checkbox(&mut app.show_finished_goals, "mostrar terminados");
    });
    ui.add_space(8.0);
    ui.separator();

    if goals.is_empty() {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.label(if filter.is_empty() && app.show_finished_goals {
                "Todavía no hay objetivos en este proyecto."
            } else {
                "No hay objetivos que coincidan con el filtro."
            });
            ui.label(
                RichText::new(
                    "Describe un resultado, no una receta: el planificador crea las tareas y \
                     el daemon las ejecuta, verifica e integra.",
                )
                .weak(),
            );
        });
        return;
    }

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for goal in &goals {
                card(ui, app, goal);
                ui.add_space(6.0);
            }
        });
}

fn composer(ui: &mut egui::Ui, app: &mut Studio) {
    let enabled = app.state.is_connected() && app.state.project().is_some();

    theme::card(false).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new("Nuevo objetivo")
                        .size(16.0)
                        .strong()
                        .color(theme::TEXT),
                );
                ui.label(
                    RichText::new(
                        "Describe el resultado y cómo se comprueba; el planificador crea el DAG.",
                    )
                    .size(11.5)
                    .color(theme::MUTED),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(
                        RichText::new("Solo conversar → Orquestador")
                            .size(11.5)
                            .color(theme::MUTED),
                    ))
                    .clicked()
                {
                    app.tab = crate::app::Tab::Orchestrator;
                }
                if !app.state.is_connected() {
                    ui.label(
                        RichText::new("daemon no disponible")
                            .size(11.5)
                            .color(theme::WARNING),
                    );
                }
            });
        });
        ui.add_space(9.0);
        let editor = ui.add_enabled(
            enabled,
            egui::TextEdit::multiline(&mut app.objective)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Body)
                .hint_text(
                    "Ej.: añadir un indicador de automatización por proyecto, con tests y sin \
                     tocar la raíz del repositorio.",
                ),
        );

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut app.plan, "Planificar en tareas")
                .on_hover_text(
                    "Desactivado: el objetivo se ejecuta como una sola tarea. Activado: el \
                     planificador crea un DAG y cada tarea se verifica por separado.",
                );
            ui.separator();
            ui.label(RichText::new("Modelo").size(12.0).color(theme::MUTED));
            crate::ui::model_selector(ui, "objective-model", &mut app.model, &app.state.models);
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Ctrl+Enter para encolar")
                    .size(11.0)
                    .color(theme::FAINT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let can_send = enabled && !app.objective.trim().is_empty();
                if ui
                    .add_enabled(
                        can_send,
                        egui::Button::new(
                            RichText::new("Encolar objetivo")
                                .size(13.0)
                                .strong()
                                .color(theme::BG),
                        )
                        .fill(theme::ACCENT)
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(egui::CornerRadius::same(7))
                        .min_size(egui::vec2(150.0, 32.0)),
                    )
                    .on_hover_text("Ctrl+Enter")
                    .clicked()
                {
                    app.queue_objective();
                }
            });
        });

        if editor.has_focus()
            && ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::Enter))
        {
            app.queue_objective();
        }
    });
}

fn card(ui: &mut egui::Ui, app: &mut Studio, goal: &GoalView) {
    let selected = app.state.selected_goal.as_deref() == Some(goal.id.as_str());
    let tasks: Vec<jcs_proto::TaskView> = app
        .state
        .goal_tasks(&goal.id)
        .into_iter()
        .cloned()
        .collect();
    let total = tasks.len();
    let succeeded = tasks
        .iter()
        .filter(|task| task_progress(task).stage == crate::state::TaskStage::Done)
        .count();
    let running = tasks
        .iter()
        .filter(|task| task_progress(task).stage.active())
        .count();
    let pending = tasks
        .iter()
        .filter(|task| task_progress(task).stage == crate::state::TaskStage::Queued)
        .count();
    let (cost, _, _) = app.state.goal_cost(&goal.id);

    let frame = theme::card(selected).show(ui, |ui| {
        ui.horizontal(|ui| {
            badge(ui, &goal.status);
            ui.label(
                RichText::new(crate::daemon::short(&goal.id))
                    .monospace()
                    .size(11.0)
                    .color(theme::FAINT),
            );
            ui.label(
                RichText::new(goal.created_at.strftime("%d %b · %H:%M").to_string())
                    .size(11.0)
                    .color(theme::FAINT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                theme::pill(theme::SURFACE_RAISED).show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("${cost:.4}"))
                            .size(12.0)
                            .strong()
                            .color(theme::TEXT),
                    );
                });
                if total > 0 {
                    ui.label(
                        RichText::new(format!("{succeeded}/{total}"))
                            .size(11.5)
                            .color(theme::MUTED),
                    );
                }
            });
        });

        ui.add_space(6.0);
        let prompt = crate::ui::truncate(&goal.prompt, 260);
        let label = ui.add(
            egui::Label::new(RichText::new(prompt).size(14.5).color(theme::TEXT))
                .sense(egui::Sense::click())
                .wrap(),
        );
        if label.clicked() {
            app.select_goal(goal.id.clone());
        }

        if total > 0 {
            let blocked = tasks
                .iter()
                .filter(|task| {
                    matches!(
                        task_progress(task).stage,
                        crate::state::TaskStage::MergeFailed
                            | crate::state::TaskStage::Blocked
                            | crate::state::TaskStage::Failed
                    )
                })
                .count();
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                stat(ui, succeeded, "completadas", theme::SUCCESS);
                stat(ui, running, "en curso", theme::ACCENT);
                stat(ui, pending, "en cola", theme::MUTED);
                stat(ui, blocked, "con problemas", theme::WARNING);
            });
            ui.add_space(6.0);

            // One clickable segment per task: the color is the real stage, and a click
            // opens its chat. No single aggregate fill to mislead.
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let width = ui.available_width().max(40.0);
                let segment = (width / total as f32).max(4.0);
                ui.horizontal(|ui| {
                    for task in &tasks {
                        let progress = task_progress(task);
                        let color = status_color(progress.stage.status_key());
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2((segment - 2.0).max(3.0), 8.0),
                            egui::Sense::click(),
                        );
                        ui.painter()
                            .rect_filled(rect, egui::CornerRadius::same(3), color);
                        if response.clicked() {
                            app.focus_task(goal.id.clone(), task.id.clone());
                        }
                        response.on_hover_text(format!(
                            "{} · {}",
                            task.title,
                            progress.stage.label()
                        ));
                    }
                });
            });

            if let Some(task) = tasks.iter().find(|task| task_progress(task).stage.active()) {
                if let Some(activity) = app.state.task_activity(&task.id) {
                    ui.add_space(7.0);
                    ui.horizontal_wrapped(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(7.0, 7.0), egui::Sense::hover());
                        ui.painter()
                            .circle_filled(rect.center(), 3.5, theme::ACCENT);
                        ui.label(
                            RichText::new(format!(
                                "{} · {} · hace {}s",
                                crate::ui::truncate(&task.title, 44),
                                activity.label(),
                                activity.age_secs()
                            ))
                            .size(11.5)
                            .color(theme::ACCENT),
                        );
                    });
                    if !activity.detail.trim().is_empty() {
                        ui.label(
                            RichText::new(crate::ui::truncate(&activity.detail, 140))
                                .size(11.5)
                                .italics()
                                .color(theme::MUTED),
                        );
                    }
                }
            }
        } else if goal.status == "running" {
            ui.add_space(6.0);
            ui.label(
                RichText::new("planificando…")
                    .size(12.0)
                    .color(theme::MUTED),
            );
        }
    });

    if ui.rect_contains_pointer(frame.response.rect)
        && ui.input(|input| input.pointer.any_click())
        && !selected
    {
        // The label above is the primary target; clicking anywhere on the card is expected.
        app.select_goal(goal.id.clone());
    }
}

fn stat(ui: &mut egui::Ui, value: usize, label: &str, color: Color32) {
    ui.label(
        RichText::new(value.to_string())
            .size(12.5)
            .strong()
            .color(color),
    );
    ui.label(RichText::new(label).size(11.0).color(theme::MUTED));
    ui.add_space(4.0);
}
