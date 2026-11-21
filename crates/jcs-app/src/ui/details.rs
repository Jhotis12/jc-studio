//! Right-hand detail: the selected objective, its tasks, runs and gates.

use egui::{Color32, RichText};

use crate::app::{ConsoleTab, Studio};
use crate::daemon::Command;
use crate::ui::{badge, metric, status_color};

pub fn goal(ui: &mut egui::Ui, app: &mut Studio) {
    let Some(goal) = app.state.selected_goal().cloned() else {
        ui.add_space(16.0);
        ui.label(
            RichText::new(
                "Selecciona un objetivo para ver sus tareas, ejecuciones y gates. Cada tarea \
                 corre en su propio worktree y solo se integra si la verificación pasa.",
            )
            .weak(),
        );
        return;
    };

    let tasks = app
        .state
        .goal_tasks(&goal.id)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let (cost, tokens_in, tokens_out) = app.state.goal_cost(&goal.id);
    let running = app.state.running_run(&goal.id).cloned();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                badge(ui, &goal.status);
                ui.label(
                    RichText::new(crate::daemon::short(&goal.id))
                        .monospace()
                        .weak()
                        .small(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button("×")
                        .on_hover_text("cerrar detalle")
                        .clicked()
                    {
                        app.state.selected_goal = None;
                    }
                });
            });

            ui.add_space(6.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label(RichText::new(&goal.prompt).size(14.0));
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                metric(ui, "coste", format!("${cost:.4}"));
                ui.separator();
                metric(ui, "tokens entrada", tokens_in.to_string());
                ui.separator();
                metric(ui, "tokens salida", tokens_out.to_string());
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if goal.status == "blocked"
                    && ui
                        .button("Reanudar objetivo")
                        .on_hover_text(
                            "Vuelve a poner el objetivo en marcha después de resolver la causa.",
                        )
                        .clicked()
                {
                    app.daemon.send(Command::Resume {
                        goal: goal.id.clone(),
                    });
                }
                if let Some(run) = &running {
                    if ui.button("Abortar ejecución").clicked() {
                        app.daemon.send(Command::Abort {
                            run: run.id.clone(),
                        });
                    }
                }
                if let Some(project) = app.state.project_by_id(&goal.project_id) {
                    if ui
                        .button("Terminal del proyecto")
                        .on_hover_text("Abre una shell en la raíz del repositorio.")
                        .clicked()
                    {
                        let _ = app.terminals.open(
                            std::path::PathBuf::from(&project.root_path),
                            project.name.clone(),
                        );
                        app.console_tab = ConsoleTab::Terminals;
                        app.show_console = true;
                    }
                }
            });

            ui.add_space(12.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Tareas").strong());
                ui.label(RichText::new(format!("{}", tasks.len())).weak());
            });
            ui.add_space(4.0);

            if tasks.is_empty() {
                ui.label(
                    RichText::new(if goal.status == "running" {
                        "El planificador todavía no ha publicado las tareas."
                    } else {
                        "Sin tareas."
                    })
                    .weak(),
                );
            }

            for task in &tasks {
                task_card(ui, app, task);
                ui.add_space(4.0);
            }
        });
}

fn task_card(ui: &mut egui::Ui, app: &mut Studio, task: &jcs_proto::TaskView) {
    let runs = app
        .state
        .task_runs(&task.id)
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();

    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.horizontal(|ui| {
            badge(ui, &task.status);
            ui.label(RichText::new(&task.title).strong());
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(crate::daemon::short(&task.id))
                    .monospace()
                    .weak()
                    .small(),
            );
            if let Some(branch) = &task.branch {
                ui.label(RichText::new(branch).monospace().weak().small());
            }
        });

        if !task.depends_on.is_empty() {
            let names = task
                .depends_on
                .iter()
                .map(|id| crate::daemon::short(id).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            ui.label(RichText::new(format!("espera a {names}")).weak().small());
        }

        if let Some(worktree) = &task.worktree {
            if ui
                .small_button("Abrir terminal en el worktree")
                .on_hover_text(worktree)
                .clicked()
            {
                let _ = app
                    .terminals
                    .open(std::path::PathBuf::from(worktree), task.title.clone());
                app.console_tab = ConsoleTab::Terminals;
                app.show_console = true;
            }
        }

        for run in &runs {
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(&run.status)
                        .small()
                        .color(status_color(&run.status)),
                );
                ui.label(RichText::new(format!("#{}", run.attempt)).weak().small());
                ui.label(
                    RichText::new(format!(
                        "${:.4} · {} in / {} out",
                        run.cost_usd, run.tokens_in, run.tokens_out
                    ))
                    .weak()
                    .small(),
                );
                if ui
                    .small_button("gates")
                    .on_hover_text("Ver los gates de esta ejecución")
                    .clicked()
                {
                    app.daemon.send(Command::RunDetail {
                        run: run.id.clone(),
                    });
                }
            });
            if let Some(error) = &run.error {
                ui.label(
                    RichText::new(error)
                        .small()
                        .color(Color32::from_rgb(225, 110, 110)),
                );
            }

            if let Some(detail) = &app.state.run_detail {
                if detail.run.id == run.id {
                    for gate in &detail.gates {
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(&gate.status)
                                    .monospace()
                                    .small()
                                    .color(status_color(&gate.status)),
                            );
                            ui.label(RichText::new(&gate.name).monospace().small());
                            ui.label(
                                RichText::new(format!("{} ms", gate.duration_ms))
                                    .weak()
                                    .small(),
                            );
                        });
                    }
                }
            }
        }
    });
}
