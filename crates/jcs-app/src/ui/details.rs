//! Right-hand detail: the selected objective, its plan graph, tasks, runs and gates.

use egui::{Color32, RichText};

use crate::app::{ConsoleTab, Studio};
use crate::daemon::Command;
use crate::state::task_progress;
use crate::ui::{badge, dag, metric, status_color};

pub fn goal(ui: &mut egui::Ui, app: &mut Studio) {
    let Some(goal) = app.state.selected_goal().cloned() else {
        ui.add_space(16.0);
        ui.label(
            RichText::new(
                "Selecciona un objetivo para ver su grafo de tareas, el progreso de cada una y \
                 lo que está haciendo el agente. Cada tarea corre en su propio worktree.",
            )
            .weak(),
        );
        return;
    };

    let tasks: Vec<jcs_proto::TaskView> = app
        .state
        .goal_tasks(&goal.id)
        .into_iter()
        .cloned()
        .collect();
    let total = tasks.len();
    let succeeded = tasks
        .iter()
        .filter(|task| task.status == "succeeded")
        .count();
    let running = tasks.iter().filter(|task| task.status == "running").count();
    let verifying = tasks
        .iter()
        .filter(|task| task.status == "verifying")
        .count();
    let pending = tasks.iter().filter(|task| task.status == "pending").count();
    let blocked = tasks.iter().filter(|task| task.status == "blocked").count();
    let failed = tasks.iter().filter(|task| task.status == "failed").count();
    let (cost, tokens_in, tokens_out) = app.state.goal_cost(&goal.id);
    let running_run = app.state.running_run(&goal.id).cloned();

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
                        app.state.selected_task = None;
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

            if total > 0 {
                ui.add_space(8.0);
                crate::ui::task_strip(ui, &tasks);
                ui.add_space(2.0);
                ui.label(
                    RichText::new(format!(
                        "{succeeded}/{total} completadas · {running} en curso ·                          {verifying} verificando · {pending} en cola · {blocked} bloqueadas ·                          {failed} fallidas"
                    ))
                    .weak()
                    .small(),
                );
            }

            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
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
                if let Some(run) = &running_run {
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

            ui.add_space(10.0);
            egui::CollapsingHeader::new("Grafo del plan")
                .default_open(true)
                .show(ui, |ui| {
                    dag::graph(ui, app, &tasks);
                });

            ui.add_space(8.0);
            ui.separator();
            let selected_task_ids = app
                .state
                .selected_task()
                .map(|task| (task.goal_id.clone(), task.id.clone()));
            ui.horizontal(|ui| {
                ui.label(RichText::new("Tareas").strong());
                ui.label(RichText::new(format!("{total}")).weak());
                if let Some((goal_id, id)) = selected_task_ids {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Ver actividad del agente").clicked() {
                            app.focus_task(goal_id, id);
                        }
                    });
                }
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

            ui.add_space(12.0);
            ui.separator();
            egui::CollapsingHeader::new("Gestionar objetivo")
                .default_open(false)
                .show(ui, |ui| {
                    let active = matches!(goal.status.as_str(), "pending" | "running" | "blocked");
                    if active {
                        if ui
                            .button("Cancelar objetivo")
                            .on_hover_text(
                                "Detiene las ejecuciones y marca las tareas como canceladas. \
                                 Conserva el objetivo y su historial.",
                            )
                            .clicked()
                        {
                            app.daemon.send(Command::CancelGoal {
                                goal: goal.id.clone(),
                            });
                        }
                        ui.label(
                            RichText::new(
                                "Cancelar no borra nada: deja el objetivo y sus tareas en el \
                                 historial, marcados como cancelados.",
                            )
                            .weak()
                            .small(),
                        );
                        ui.add_space(6.0);
                    }

                    if matches!(goal.status.as_str(), "failed" | "cancelled" | "blocked")
                        && ui
                            .button("Reintentar como objetivo nuevo")
                            .on_hover_text(
                                "Crea un objetivo nuevo con el mismo texto. No reutiliza las \
                             tareas fallidas.",
                            )
                            .clicked()
                    {
                        app.retry_goal(&goal.id);
                    }

                    let has_running = tasks
                        .iter()
                        .any(|task| matches!(task.status.as_str(), "running" | "verifying"));
                    ui.checkbox(
                        &mut app.confirm_delete_goal,
                        "confirmo que quiero eliminar este objetivo",
                    );
                    if ui
                        .add_enabled(
                            app.confirm_delete_goal && !has_running,
                            egui::Button::new("Eliminar objetivo"),
                        )
                        .on_hover_text(
                            "Elimina el objetivo, sus tareas y sus runs. Los eventos de \
                             auditoría se conservan.",
                        )
                        .clicked()
                    {
                        app.daemon.send(Command::DeleteGoal {
                            goal: goal.id.clone(),
                        });
                    }
                    if has_running {
                        ui.label(
                            RichText::new(
                                "Hay tareas en ejecución: cancela el objetivo y espera a que \
                                 se detengan antes de eliminarlo.",
                            )
                            .small()
                            .color(Color32::from_rgb(230, 170, 90)),
                        );
                    }
                });
        });
}

fn task_card(ui: &mut egui::Ui, app: &mut Studio, task: &jcs_proto::TaskView) {
    let runs: Vec<jcs_proto::RunView> =
        app.state.task_runs(&task.id).into_iter().cloned().collect();
    let selected = app.state.selected_task.as_deref() == Some(task.id.as_str());
    let progress = task_progress(task);
    let elapsed = app.state.task_elapsed_secs(task);
    let tokens = app.state.task_tokens(&task.id);

    let frame = egui::Frame::group(ui.style()).fill(if selected {
        ui.visuals().selection.bg_fill.gamma_multiply(0.18)
    } else {
        ui.visuals().faint_bg_color
    });

    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            badge(ui, &task.status);
            if task.merge_error.is_some() {
                badge(ui, "merge bloqueado");
            }
            if ui
                .selectable_label(selected, RichText::new(&task.title).strong())
                .on_hover_text("Ver la actividad del agente para esta tarea")
                .clicked()
            {
                app.focus_task(task.goal_id.clone(), task.id.clone());
            }
        });

        ui.add_space(3.0);
        ui.horizontal(|ui| {
            crate::ui::stage_steps(ui, progress, 120.0);
            ui.label(
                RichText::new(format!("{} · {} tok", duration(elapsed), tokens))
                    .weak()
                    .small(),
            );
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
            ui.label(RichText::new(format!("depende de {names}")).weak().small());
        }
        if let Some(reason) = app.state.task_wait_reason(task) {
            ui.label(
                RichText::new(format!("en espera · {reason}"))
                    .small()
                    .color(Color32::from_rgb(230, 170, 90)),
            );
        }
        if let Some(error) = &task.merge_error {
            ui.label(
                RichText::new(format!("integración fallida: {error}"))
                    .small()
                    .color(Color32::from_rgb(225, 110, 110)),
            );
            if ui
                .small_button("Reintentar integración")
                .on_hover_text(
                    "Resuelve el conflicto en el worktree y pulsa aquí para que el merge \
                     se reintente.",
                )
                .clicked()
            {
                app.daemon.send(Command::RetryMerge {
                    task: task.id.clone(),
                });
            }
        }

        ui.horizontal_wrapped(|ui| {
            if ui
                .small_button("Chat del agente")
                .on_hover_text("Razonamiento, respuesta y herramientas en vivo.")
                .clicked()
            {
                app.focus_task(task.goal_id.clone(), task.id.clone());
            }
            let iterable = matches!(
                task.status.as_str(),
                "succeeded" | "failed" | "blocked" | "pending"
            );
            if iterable {
                let label = match task.status.as_str() {
                    "succeeded" => "Pedir cambios",
                    "pending" => "Añadir indicaciones",
                    _ => "Reintentar con indicaciones",
                };
                if ui
                    .small_button(label)
                    .on_hover_text(if task.status == "succeeded" {
                        "Crea una tarea dependiente con tus indicaciones; el trabajo original \
                         no se modifica."
                    } else {
                        "Vuelve a poner la tarea en cola con las indicaciones nuevas."
                    })
                    .clicked()
                {
                    app.iteration_for = Some(task.id.clone());
                    app.iteration_feedback.clear();
                }
            }
            if let Some(worktree) = &task.worktree {
                if ui
                    .small_button("Terminal en el worktree")
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
        });

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

fn duration(seconds: i64) -> String {
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}
