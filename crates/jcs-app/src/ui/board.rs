//! The objectives board: compose a goal, then watch its DAG move.
//!
//! The composer is the only way to start work; there is no command to remember. Execution
//! settings stay in the Settings window, so writing an objective is one text box.

use egui::{Color32, RichText};
use jcs_proto::GoalView;

use crate::app::Studio;
use crate::ui::{badge, status_color};

pub fn goals(ui: &mut egui::Ui, app: &mut Studio) {
    composer(ui, app);
    ui.add_space(8.0);
    ui.separator();

    let goals: Vec<GoalView> = app.state.goals().into_iter().cloned().collect();
    if goals.is_empty() {
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.label("Todavía no hay objetivos en este proyecto.");
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

    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Nuevo objetivo").strong());
            if !app.state.is_connected() {
                ui.colored_label(
                    Color32::from_rgb(230, 170, 90),
                    "el daemon no está disponible",
                );
            }
        });
        ui.add_space(4.0);
        let editor = ui.add_enabled(
            enabled,
            egui::TextEdit::multiline(&mut app.objective)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text(
                    "Qué debe conseguirse y cómo se comprueba. El planificador lo divide en \
                     tareas con dependencias.",
                ),
        );

        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut app.plan, "Planificar en tareas")
                .on_hover_text(
                    "Desactivado: el objetivo se ejecuta como una sola tarea. Activado: el \
                     planificador crea un DAG y cada tarea se verifica por separado.",
                );
            ui.separator();
            ui.label("Modelo");
            crate::ui::model_selector(ui, "objective-model", &mut app.model, &app.state.models);
        });
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let can_send = enabled && !app.objective.trim().is_empty();
                if ui
                    .add_enabled(can_send, egui::Button::new("Encolar objetivo"))
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
    let total = app.state.goal_tasks(&goal.id).len();
    let succeeded = app
        .state
        .goal_tasks(&goal.id)
        .iter()
        .filter(|task| task.status == "succeeded")
        .count();
    let running = app
        .state
        .goal_tasks(&goal.id)
        .iter()
        .any(|task| task.status == "running");
    let (cost, _, _) = app.state.goal_cost(&goal.id);

    let frame = egui::Frame::group(ui.style()).fill(if selected {
        ui.visuals().selection.bg_fill.gamma_multiply(0.25)
    } else {
        ui.visuals().faint_bg_color
    });

    let frame = frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            badge(ui, &goal.status);
            ui.label(
                RichText::new(crate::daemon::short(&goal.id))
                    .monospace()
                    .weak()
                    .small(),
            );
            ui.label(
                RichText::new(goal.created_at.strftime("%d %b %H:%M").to_string())
                    .weak()
                    .small(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new(format!("${cost:.4}")).strong().small());
                if total > 0 {
                    ui.label(
                        RichText::new(format!("{succeeded}/{total} tareas"))
                            .weak()
                            .small(),
                    );
                }
            });
        });

        let prompt = crate::ui::truncate(&goal.prompt, 240);
        let label = ui.add(
            egui::Label::new(RichText::new(prompt).size(14.0))
                .sense(egui::Sense::click())
                .wrap(),
        );
        if label.clicked() {
            app.select_goal(goal.id.clone());
        }

        if total > 0 {
            let fraction = succeeded as f32 / total as f32;
            ui.add_space(2.0);
            ui.add(
                egui::ProgressBar::new(fraction)
                    .desired_width(f32::INFINITY)
                    .desired_height(6.0)
                    .fill(if running {
                        status_color("running")
                    } else {
                        status_color("succeeded")
                    })
                    .text(RichText::new("").small()),
            );
        } else if goal.status == "running" {
            ui.horizontal(|ui| {
                ui.label(RichText::new("planificando…").weak().small());
            });
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
