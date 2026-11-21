//! System panel: daemon health and the paths that explain where state lives.

use egui::{Color32, RichText};

use crate::app::Studio;
use crate::ui::metric;

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
                        .small(),
                );
                ui.label("O en primer plano, para ver los logs:");
                ui.label(RichText::new("jcsd").monospace().small());
                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "La aplicación reintenta la conexión cada dos segundos; puedes dejarla \
                         abierta mientras arrancas el servicio.",
                    )
                    .weak(),
                );
                return;
            };

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                metric(ui, "versión", health.version.clone());
                ui.separator();
                metric(ui, "protocolo", health.protocol.to_string());
                ui.separator();
                metric(ui, "uptime", format!("{} s", health.uptime_secs));
                ui.separator();
                metric(
                    ui,
                    "coste total",
                    format!("${:.4}", app.state.totals.cost_usd),
                );
            });

            ui.add_space(10.0);
            ui.separator();
            ui.label(RichText::new("Agente").strong());
            egui::Grid::new("agent")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("backend").weak());
                    ui.label(RichText::new(&health.agent.backend).monospace().small());
                    ui.end_row();
                    ui.label(RichText::new("estado").weak());
                    if health.agent.reachable {
                        ui.label(
                            RichText::new("disponible")
                                .color(Color32::from_rgb(120, 200, 130))
                                .small(),
                        );
                    } else {
                        ui.label(
                            RichText::new("no disponible")
                                .color(Color32::from_rgb(225, 110, 110))
                                .small(),
                        );
                    }
                    ui.end_row();
                    if let Some(url) = &health.agent.base_url {
                        ui.label(RichText::new("url").weak());
                        ui.label(RichText::new(url).monospace().small());
                        ui.end_row();
                    }
                    if let Some(version) = &health.agent.version {
                        ui.label(RichText::new("versión").weak());
                        ui.label(RichText::new(version).monospace().small());
                        ui.end_row();
                    }
                    if let Some(message) = &health.agent.message {
                        ui.label(RichText::new("detalle").weak());
                        ui.label(RichText::new(message).small());
                        ui.end_row();
                    }
                });

            ui.add_space(10.0);
            ui.separator();
            ui.label(RichText::new("Rutas y estado").strong());
            egui::Grid::new("paths")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("socket").weak());
                    ui.label(
                        RichText::new(jcs_proto::paths::socket_path().display().to_string())
                            .monospace()
                            .small(),
                    );
                    ui.end_row();
                    ui.label(RichText::new("base de datos").weak());
                    ui.label(RichText::new(&health.database).monospace().small());
                    ui.end_row();
                    ui.label(RichText::new("worktrees").weak());
                    ui.label(
                        RichText::new(jcs_proto::paths::worktrees_dir().display().to_string())
                            .monospace()
                            .small(),
                    );
                    ui.end_row();
                    ui.label(RichText::new("proyectos").weak());
                    ui.label(app.state.totals.projects.to_string());
                    ui.end_row();
                    ui.label(RichText::new("objetivos").weak());
                    ui.label(app.state.totals.goals.to_string());
                    ui.end_row();
                    ui.label(RichText::new("tareas").weak());
                    ui.label(app.state.totals.tasks.to_string());
                    ui.end_row();
                    ui.label(RichText::new("tokens").weak());
                    ui.label(format!(
                        "{} in / {} out",
                        app.state.totals.tokens_in, app.state.totals.tokens_out
                    ));
                    ui.end_row();
                });

            ui.add_space(10.0);
            ui.separator();
            ui.label(RichText::new("Observabilidad").strong());
            ui.label(
                RichText::new(
                    "Métricas Prometheus en el socket del daemon (`jcs metrics`) y logs en \
                     journalctl si corre como servicio. El RUNBOOK documenta la recuperación.",
                )
                .weak()
                .small(),
            );
        });
}
