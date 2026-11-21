//! Policy: the compiled packs, gates and architectural boundaries of the project.

use egui::{Color32, RichText};

use crate::app::Studio;
use crate::daemon::Command;
use crate::ui::{badge, status_color};

pub fn panel(ui: &mut egui::Ui, app: &mut Studio) {
    let Some(project) = app.state.project().cloned() else {
        ui.label("Selecciona un proyecto.");
        return;
    };

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Política ejecutable").strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Ejecutar gates").clicked() {
                app.daemon.send(Command::RunGates {
                    project: project.name.clone(),
                    gate: None,
                });
            }
            if ui
                .button("Comprobar drift")
                .on_hover_text("Falla si los archivos renderizados no coinciden con la política.")
                .clicked()
            {
                app.daemon.send(Command::RenderPolicy {
                    project: project.name.clone(),
                    check: true,
                });
            }
            if ui
                .button("Renderizar")
                .on_hover_text("Escribe AGENTS.md y los archivos de política en el repositorio.")
                .clicked()
            {
                app.daemon.send(Command::RenderPolicy {
                    project: project.name.clone(),
                    check: false,
                });
            }
            if ui.button("Recargar").clicked() {
                app.daemon.send(Command::Policy {
                    project: project.name.clone(),
                });
            }
        });
    });
    ui.add_space(4.0);

    let Some(policy) = app.state.policy.clone() else {
        ui.label(RichText::new("Cargando política…").weak());
        return;
    };

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if !policy.drift.is_empty() {
                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(225, 110, 110).gamma_multiply(0.12))
                    .show(ui, |ui| {
                        ui.colored_label(
                            Color32::from_rgb(235, 130, 130),
                            format!("{} archivos con drift", policy.drift.len()),
                        );
                        for path in &policy.drift {
                            ui.label(RichText::new(path).monospace().small());
                        }
                    });
                ui.add_space(6.0);
            }

            ui.label(RichText::new("Packs").strong());
            if policy.packs.is_empty() {
                ui.label(RichText::new("Sin packs: solo los gates de commit integrados.").weak());
            }
            for pack in &policy.packs {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&pack.id).monospace().strong());
                    ui.label(RichText::new(format!("v{}", pack.version)).weak().small());
                    ui.label(
                        RichText::new(&pack.hash[..8.min(pack.hash.len())])
                            .weak()
                            .small(),
                    );
                    ui.label(RichText::new(&pack.description).small());
                });
            }

            ui.add_space(8.0);
            ui.label(RichText::new("Gates").strong());
            for gate in &policy.gates {
                ui.horizontal_wrapped(|ui| {
                    badge(
                        ui,
                        if gate.required {
                            "required"
                        } else {
                            "optional"
                        },
                    );
                    ui.label(RichText::new(&gate.name).monospace().strong());
                    ui.label(
                        RichText::new(gate.command.join(" "))
                            .monospace()
                            .weak()
                            .small(),
                    );
                    ui.label(RichText::new(format!("de {}", gate.source)).weak().small());
                });
            }

            ui.add_space(8.0);
            ui.label(RichText::new("Reglas").strong());
            egui::Grid::new("rules")
                .num_columns(3)
                .spacing([12.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(RichText::new("clave").weak().small());
                    ui.label(RichText::new("valor").weak().small());
                    ui.label(RichText::new("origen").weak().small());
                    ui.end_row();
                    for rule in &policy.rules {
                        ui.label(RichText::new(&rule.key).monospace().small());
                        ui.label(RichText::new(&rule.value).monospace().small());
                        ui.label(RichText::new(&rule.source).weak().small());
                        ui.end_row();
                    }
                });

            if !policy.skills.is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new("Skills").strong());
                ui.label(RichText::new(policy.skills.join(", ")).monospace().small());
            }

            if let Some(rendered) = &app.state.rendered {
                ui.add_space(8.0);
                ui.separator();
                ui.label(RichText::new("Última renderización").strong());
                for file in &rendered.files {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&file.state).monospace().small().color(
                            status_color(match file.state.as_str() {
                                "created" | "updated" | "unchanged" => "succeeded",
                                "refused" | "missing" => "blocked",
                                _ => "failed",
                            }),
                        ));
                        ui.label(RichText::new(&file.path).monospace().small());
                        if let Some(detail) = &file.detail {
                            ui.label(RichText::new(detail).weak().small());
                        }
                    });
                }
            }

            if let Some(gates) = &app.state.gates {
                ui.add_space(8.0);
                ui.separator();
                ui.label(RichText::new("Última ejecución de gates").strong());
                for outcome in &gates.outcomes {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&outcome.status)
                                .monospace()
                                .small()
                                .color(status_color(&outcome.status)),
                        );
                        ui.label(RichText::new(&outcome.name).monospace().small());
                        ui.label(
                            RichText::new(format!("{} ms", outcome.duration_ms))
                                .weak()
                                .small(),
                        );
                    });
                    if !outcome.excerpt.trim().is_empty() {
                        ui.label(
                            RichText::new(outcome.excerpt.trim())
                                .monospace()
                                .small()
                                .weak(),
                        );
                    }
                    for finding in &outcome.findings {
                        ui.label(
                            RichText::new(finding)
                                .small()
                                .color(Color32::from_rgb(235, 160, 120)),
                        );
                    }
                }
            }
        });
}
