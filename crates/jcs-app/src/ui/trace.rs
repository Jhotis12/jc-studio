//! Live agent trace for one task: reasoning, answer, tool calls and system milestones.
//!
//! The event log is the source of truth. When a task is selected after it started, the
//! application requests its bounded history; afterwards the global stream keeps it current.

use egui::{Color32, RichText};

use crate::app::Studio;
use crate::daemon::Command;
use crate::state::{task_progress, ActivityLine};
use crate::ui::{badge, status_color, truncate};

pub fn panel(ui: &mut egui::Ui, app: &mut Studio) {
    let Some(task) = app.state.selected_task().cloned() else {
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Selecciona una tarea en el detalle o en el grafo del plan para ver qué está \
                 haciendo el agente: razonamiento, respuesta, herramientas y gates.",
            )
            .weak(),
        );
        return;
    };

    let progress = task_progress(&task);
    let tokens = app.state.task_tokens(&task.id);
    let elapsed = app.state.task_elapsed_secs(&task);

    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&task.title).strong());
        badge(ui, &task.status);
        ui.add(
            egui::ProgressBar::new(progress.fraction)
                .desired_width(110.0)
                .desired_height(6.0)
                .fill(status_color(&task.status))
                .text(""),
        );
        ui.label(
            RichText::new(format!(
                "{}% · {} · {} · {} tok",
                (progress.fraction * 100.0).round() as i32,
                progress.label,
                duration(elapsed),
                tokens
            ))
            .weak()
            .small(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button("Actualizar")
                .on_hover_text("Vuelve a pedir la historia de esta tarea.")
                .clicked()
            {
                app.daemon.send(Command::TaskEvents {
                    task: task.id.clone(),
                });
            }
        });
    });
    ui.separator();

    let lines: Vec<ActivityLine> = app
        .state
        .events
        .iter()
        .filter(|line| line.task_id.as_deref() == Some(task.id.as_str()))
        .cloned()
        .collect();

    if lines.is_empty() {
        ui.add_space(8.0);
        ui.label(
            RichText::new(if task.status == "pending" {
                "En cola: todavía no hay actividad del agente para esta tarea."
            } else {
                "Sin actividad registrada todavía. El stream se actualiza en vivo."
            })
            .weak(),
        );
        return;
    }

    // A finished task is read from the beginning; a live one should follow the tail.
    let live = matches!(task.status.as_str(), "running" | "verifying");
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(live)
        .show(ui, |ui| {
            for line in &lines {
                trace_line(ui, line);
            }
        });
}

fn trace_line(ui: &mut egui::Ui, line: &ActivityLine) {
    match line.kind.as_str() {
        "agent_reasoning" => {
            block(
                ui,
                Color32::from_rgb(120, 130, 155),
                "pensando",
                &line.detail,
                true,
            );
        }
        "agent_text" => {
            block(
                ui,
                Color32::from_rgb(190, 205, 230),
                "agente",
                &line.detail,
                false,
            );
        }
        "agent_tool_called" | "agent_tool_finished" => {
            let color = if line.kind == "agent_tool_finished" {
                Color32::from_rgb(120, 200, 130)
            } else {
                Color32::from_rgb(105, 165, 235)
            };
            block(ui, color, &line.kind_label(), &line.detail, false);
        }
        _ => {
            let color = status_color(match line.kind.as_str() {
                "error" => "failed",
                "run_finished" | "commit_created" | "merge_finished" => "succeeded",
                _ => "running",
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&line.time).monospace().weak().small());
                ui.label(RichText::new(&line.kind).monospace().small().color(color));
                if !line.detail.is_empty() {
                    ui.label(
                        RichText::new(truncate(&line.detail, 240))
                            .monospace()
                            .small()
                            .weak(),
                    );
                }
            });
        }
    }
}

fn block(ui: &mut egui::Ui, color: Color32, label: &str, text: &str, italic: bool) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.08))
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(label).small().strong().color(color));
            });
            let mut rich = RichText::new(text.trim()).monospace().size(12.5);
            if italic {
                rich = rich.italics();
            }
            ui.label(rich.color(color));
        });
    ui.add_space(2.0);
}

impl ActivityLine {
    fn kind_label(&self) -> String {
        if self.kind == "agent_tool_finished" {
            "resultado".to_string()
        } else {
            "herramienta".to_string()
        }
    }
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
