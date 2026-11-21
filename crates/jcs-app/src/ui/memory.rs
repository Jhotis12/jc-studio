//! Memory: what the agents can recall about this repository, and the cited search.

use egui::{Color32, RichText};

use crate::app::Studio;
use crate::daemon::Command;
use crate::ui::metric;

pub fn panel(ui: &mut egui::Ui, app: &mut Studio) {
    let Some(project) = app.state.project().cloned() else {
        ui.label("Selecciona un proyecto.");
        return;
    };

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Memoria del proyecto").strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("Reindexar")
                .on_hover_text("Ignora la caché por hash y vuelve a indexar todo.")
                .clicked()
            {
                app.daemon.send(Command::IndexMemory {
                    project: project.name.clone(),
                    force: true,
                });
            }
            if ui.button("Indexar cambios").clicked() {
                app.daemon.send(Command::IndexMemory {
                    project: project.name.clone(),
                    force: false,
                });
            }
            if ui.button("Actualizar").clicked() {
                app.daemon.send(Command::MemoryStats {
                    project: project.name.clone(),
                });
            }
        });
    });
    ui.add_space(4.0);

    match &app.state.memory {
        Some(stats) => {
            ui.horizontal(|ui| {
                metric(ui, "archivos", stats.files.to_string());
                ui.separator();
                metric(ui, "símbolos", stats.symbols.to_string());
                ui.separator();
                metric(ui, "chunks", stats.chunks.to_string());
                ui.separator();
                metric(ui, "con embeddings", stats.embedded_chunks.to_string());
                ui.separator();
                metric(ui, "aristas", stats.edges.to_string());
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(match &stats.last_indexed_at {
                        Some(at) => format!("última indexación {at}"),
                        None => "sin indexar todavía".to_string(),
                    })
                    .weak()
                    .small(),
                );
                ui.separator();
                if stats.semantic_enabled {
                    ui.label(
                        RichText::new("búsqueda semántica activa")
                            .small()
                            .color(Color32::from_rgb(120, 200, 130)),
                    );
                } else {
                    ui.label(
                        RichText::new("búsqueda semántica desactivada (sin embeddings)")
                            .small()
                            .weak(),
                    );
                }
            });
        }
        None => {
            ui.label(RichText::new("Sin estadísticas; pulsa Actualizar.").weak());
        }
    }

    if let Some(report) = &app.state.memory_index {
        ui.add_space(6.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "Indexación: {} vistos · {} indexados · {} sin cambios · {} eliminados · \
                     {} símbolos · {} chunks · {} embeddings · {} ms",
                    report.report.files_seen,
                    report.report.files_indexed,
                    report.report.files_skipped,
                    report.report.files_removed,
                    report.report.symbols,
                    report.report.chunks,
                    report.report.embeddings,
                    report.report.duration_ms
                ))
                .small(),
            );
            for error in &report.report.errors {
                ui.label(
                    RichText::new(error)
                        .small()
                        .color(Color32::from_rgb(225, 110, 110)),
                );
            }
        });
    }

    ui.add_space(8.0);
    ui.separator();
    ui.label(RichText::new("Buscar en el código").strong());
    ui.horizontal(|ui| {
        let response = ui.add(
            egui::TextEdit::singleline(&mut app.memory_query)
                .hint_text("¿dónde se valida el presupuesto?")
                .desired_width(f32::INFINITY),
        );
        let submit = ui.button("Buscar").clicked()
            || (response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
        if submit && !app.memory_query.trim().is_empty() {
            app.daemon.send(Command::SearchMemory {
                project: project.name.clone(),
                query: app.memory_query.trim().to_string(),
            });
        }
    });
    ui.add_space(4.0);

    match &app.state.memory_search {
        Some(search) if !search.hits.is_empty() => {
            ui.label(
                RichText::new(format!(
                    "{} resultados para «{}»",
                    search.hits.len(),
                    search.query
                ))
                .weak()
                .small(),
            );
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for hit in &search.hits {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&hit.citation).monospace().strong());
                                ui.label(RichText::new(format!("{:.3}", hit.score)).weak().small());
                                ui.label(RichText::new(&hit.source).weak().small());
                            });
                            let short = crate::ui::truncate(hit.snippet.trim(), 600);
                            ui.label(RichText::new(short).monospace().small());
                        });
                        ui.add_space(4.0);
                    }
                });
        }
        Some(search) => {
            ui.label(RichText::new(format!("Sin resultados para «{}».", search.query)).weak());
        }
        None => {}
    }
}
