//! JC Studio desktop application.
//!
//! An installable control plane for multi-agent work: projects, objectives, gates, memory
//! and embedded terminals. The daemon (`jcsd`) owns all durable state; closing this window
//! cannot lose work.

mod app;
mod daemon;
mod state;
mod terminal;
mod ui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([980.0, 640.0])
            .with_app_id("jc-studio")
            .with_title("JC Studio"),
        ..Default::default()
    };

    eframe::run_native(
        "JC Studio",
        options,
        Box::new(|context| {
            ui::theme::apply(&context.egui_ctx);
            Ok(Box::new(app::Studio::new()))
        }),
    )
}
