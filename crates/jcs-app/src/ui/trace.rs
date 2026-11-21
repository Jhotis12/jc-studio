//! Chat view for one agent: the objective, the task, the answer, the reasoning and the
//! tools. The event log is the source; chunks are coalesced so a streamed answer reads as
//! one message instead of one bubble per 200 ms.

use egui::RichText;

use crate::app::Studio;
use crate::daemon::Command;
use crate::state::{task_progress, ActivityLine};
use crate::ui::{badge, status_color, theme, truncate};

enum Item {
    Text(String),
    Reasoning(String),
    Tool { failed: bool, detail: String },
    System(ActivityLine),
}

pub fn panel(ui: &mut egui::Ui, app: &mut Studio) {
    let Some(task) = app.state.selected_task().cloned() else {
        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "Selecciona una tarea en el detalle o en el grafo del plan para abrir su chat: \
                 objetivo, razonamiento, respuesta del agente, herramientas y gates.",
            )
            .weak(),
        );
        return;
    };

    let goal_prompt = app
        .state
        .selected_goal()
        .map(|goal| goal.prompt.clone())
        .unwrap_or_default();
    let progress = task_progress(&task);
    let tokens = app.state.task_tokens(&task.id);
    let elapsed = app.state.task_elapsed_secs(&task);

    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Chat").size(14.0).strong().color(theme::TEXT));
        badge(ui, &task.status);
        crate::ui::stage_steps(ui, progress, 100.0);
        ui.label(
            RichText::new(format!("{} · {} tok", duration(elapsed), tokens))
                .size(11.0)
                .color(theme::FAINT),
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
    let items = coalesce(lines);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(matches!(task.status.as_str(), "running" | "verifying"))
        .show(ui, |ui| {
            user_message(ui, "Objetivo", &goal_prompt);
            user_message(ui, "Tarea", &task.title);

            if items.is_empty() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(if task.status == "pending" {
                        "En cola: el agente todavía no ha empezado esta tarea."
                    } else {
                        "Sin actividad registrada todavía; el chat se actualiza en vivo."
                    })
                    .size(13.0)
                    .color(theme::MUTED),
                );
            }

            for item in &items {
                match item {
                    Item::Text(text) => assistant_message(ui, text),
                    Item::Reasoning(text) => reasoning(ui, text),
                    Item::Tool { failed, detail } => tool(ui, *failed, detail),
                    Item::System(line) => system_line(ui, line),
                }
            }
        });
}

/// Concatenates adjacent streamed chunks of the same kind.
fn coalesce(lines: Vec<ActivityLine>) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    for line in lines {
        match line.kind.as_str() {
            "agent_text" => match items.last_mut() {
                Some(Item::Text(text)) => text.push_str(&line.detail),
                _ => items.push(Item::Text(line.detail)),
            },
            "agent_reasoning" => match items.last_mut() {
                Some(Item::Reasoning(text)) => text.push_str(&line.detail),
                _ => items.push(Item::Reasoning(line.detail)),
            },
            "agent_tool_called" => items.push(Item::Tool {
                failed: false,
                detail: line.detail,
            }),
            "agent_tool_finished" => items.push(Item::Tool {
                failed: line.failed,
                detail: line.detail,
            }),
            _ => items.push(Item::System(line)),
        }
    }
    items
}

fn user_message(ui: &mut egui::Ui, label: &str, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(theme::ACCENT_SOFT.gamma_multiply(0.55))
        .corner_radius(theme::RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(11, 8))
        .show(ui, |ui| {
            ui.label(
                RichText::new(label)
                    .size(11.0)
                    .strong()
                    .color(theme::ACCENT),
            );
            ui.label(RichText::new(text).size(14.0).color(theme::TEXT));
        });
    ui.add_space(3.0);
}

fn assistant_message(ui: &mut egui::Ui, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    egui::Frame::new()
        .fill(theme::SURFACE_RAISED)
        .corner_radius(theme::RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(11, 8))
        .show(ui, |ui| {
            ui.label(
                RichText::new("Agente")
                    .size(11.0)
                    .strong()
                    .color(theme::MUTED),
            );
            crate::ui::markdown::render(ui, text.trim());
        });
    ui.add_space(3.0);
}

fn reasoning(ui: &mut egui::Ui, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    egui::CollapsingHeader::new(RichText::new("Pensamiento").size(11.0).color(theme::FAINT))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(
                RichText::new(truncate(text.trim(), 8_000))
                    .italics()
                    .size(12.5)
                    .color(theme::MUTED),
            );
        });
}

fn tool(ui: &mut egui::Ui, failed: bool, detail: &str) {
    let color = if failed { theme::DANGER } else { theme::ACCENT };
    let icon = if failed { "✗" } else { "→" };
    egui::Frame::new()
        .fill(color.gamma_multiply(0.09))
        .corner_radius(theme::RADIUS_SM)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("{icon} {detail}"))
                    .monospace()
                    .size(12.0)
                    .color(color),
            );
        });
    ui.add_space(2.0);
}

fn system_line(ui: &mut egui::Ui, line: &ActivityLine) {
    let color = status_color(match line.kind.as_str() {
        "error" => "failed",
        "run_finished" | "commit_created" | "merge_finished" => "succeeded",
        _ => "running",
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(&line.time)
                .monospace()
                .size(11.0)
                .color(theme::FAINT),
        );
        ui.label(
            RichText::new(&line.kind)
                .monospace()
                .size(11.0)
                .color(color),
        );
        if !line.detail.is_empty() {
            ui.label(
                RichText::new(truncate(&line.detail, 240))
                    .monospace()
                    .size(11.0)
                    .color(theme::MUTED),
            );
        }
        if line.count > 1 {
            ui.label(
                RichText::new(format!("×{}", line.count))
                    .size(11.0)
                    .color(theme::FAINT),
            );
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
