//! System panel: daemon health and the paths that explain where state lives.

use egui::RichText;

use crate::app::Studio;
use crate::ui::{metric, theme};

pub fn panel(ui: &mut egui::Ui, app: &mut Studio) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let Some(health) = app.state.health.clone() else {
                ui.add_space(12.0);
                ui.label(RichText::new("El daemon no está disponible.").heading());
                ui.add_space(8.0);
                ui.label("Arráncalo como servicio de usuario:");
                ui.label(
                    RichText::new("systemctl --user enable --now jcsd")
                        .monospace()
                        .size(12.0)
                        .color(theme::MUTED),
                );
                ui.label("O en primer plano, para ver los logs:");
                ui.label(
                    RichText::new("jcsd")
                        .monospace()
                        .size(12.0)
                        .color(theme::MUTED),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "La aplicación reintenta la conexión cada dos segundos; puedes dejarla \
                         abierta mientras arrancas el servicio.",
                    )
                    .size(13.0)
                    .color(theme::MUTED),
                );
                return;
            };

            theme::card(false).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    metric(ui, "versión", health.version.clone());
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);
                    metric(ui, "protocolo", health.protocol.to_string());
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);
                    metric(ui, "uptime", format!("{} s", health.uptime_secs));
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);
                    metric(
                        ui,
                        "coste total",
                        format!("${:.4}", app.state.totals.cost_usd),
                    );
                });
            });

            ui.add_space(10.0);
            ui.columns(2, |columns| {
                theme::card(false).show(&mut columns[0], |ui| {
                    section(ui, "Agente");
                    ui.add_space(4.0);
                    egui::Grid::new("agent")
                        .num_columns(2)
                        .spacing([14.0, 6.0])
                        .show(ui, |ui| {
                            key(ui, "backend");
                            value(ui, &health.agent.backend);
                            ui.end_row();
                            key(ui, "estado");
                            if health.agent.reachable {
                                ui.label(
                                    RichText::new("disponible").size(12.0).color(theme::SUCCESS),
                                );
                            } else {
                                ui.label(
                                    RichText::new("no disponible")
                                        .size(12.0)
                                        .color(theme::DANGER),
                                );
                            }
                            ui.end_row();
                            if let Some(url) = &health.agent.base_url {
                                key(ui, "url");
                                value(ui, url);
                                ui.end_row();
                            }
                            if let Some(version) = &health.agent.version {
                                key(ui, "versión");
                                value(ui, version);
                                ui.end_row();
                            }
                            if let Some(message) = &health.agent.message {
                                key(ui, "detalle");
                                ui.label(RichText::new(message).size(11.5).color(theme::MUTED));
                                ui.end_row();
                            }
                        });
                });

                theme::card(false).show(&mut columns[1], |ui| {
                    section(ui, "Volumen de trabajo");
                    ui.add_space(4.0);
                    egui::Grid::new("totals")
                        .num_columns(2)
                        .spacing([14.0, 6.0])
                        .show(ui, |ui| {
                            key(ui, "proyectos");
                            value(ui, &app.state.totals.projects.to_string());
                            ui.end_row();
                            key(ui, "objetivos");
                            value(ui, &app.state.totals.goals.to_string());
                            ui.end_row();
                            key(ui, "tareas");
                            value(ui, &app.state.totals.tasks.to_string());
                            ui.end_row();
                            key(ui, "tokens");
                            value(
                                ui,
                                &format!(
                                    "{} in / {} out",
                                    app.state.totals.tokens_in, app.state.totals.tokens_out
                                ),
                            );
                            ui.end_row();
                        });
                });
            });

            ui.add_space(10.0);
            theme::card(false).show(ui, |ui| {
                section(ui, "Rutas y estado");
                ui.add_space(4.0);
                egui::Grid::new("paths")
                    .num_columns(2)
                    .spacing([14.0, 6.0])
                    .show(ui, |ui| {
                        key(ui, "socket");
                        value(ui, &jcs_proto::paths::socket_path().display().to_string());
                        ui.end_row();
                        key(ui, "base de datos");
                        value(ui, &health.database);
                        ui.end_row();
                        key(ui, "worktrees");
                        value(ui, &jcs_proto::paths::worktrees_dir().display().to_string());
                        ui.end_row();
                    });
            });

            ui.add_space(10.0);
            theme::card(false).show(ui, |ui| {
                section(ui, "Observabilidad");
                ui.add_space(3.0);
                ui.label(
                    RichText::new(
                        "Métricas Prometheus en el socket del daemon (`jcs metrics`) y logs en \
                         journalctl si corre como servicio. El RUNBOOK documenta la recuperación.",
                    )
                    .size(12.5)
                    .color(theme::MUTED),
                );
            });
        });
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(RichText::new(title).size(14.0).strong().color(theme::TEXT));
}

fn key(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(11.5).color(theme::FAINT));
}

fn value(ui: &mut egui::Ui, text: &str) {
    ui.add(
        egui::Label::new(
            RichText::new(text)
                .monospace()
                .size(11.5)
                .color(theme::MUTED),
        )
        .truncate(),
    );
}
