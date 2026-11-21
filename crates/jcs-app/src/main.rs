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
            style(&context.egui_ctx);
            Ok(Box::new(app::Studio::new()))
        }),
    )
}

/// Dark, compact and monospace-friendly: this is an operator console, not a marketing page.
fn style(context: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(18, 20, 25);
    visuals.window_fill = egui::Color32::from_rgb(22, 24, 30);
    visuals.extreme_bg_color = egui::Color32::from_rgb(12, 14, 18);
    visuals.faint_bg_color = egui::Color32::from_rgb(24, 27, 33);
    visuals.selection.bg_fill = egui::Color32::from_rgb(48, 84, 130);
    context.set_visuals(visuals);

    let mut style = (*context.style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    context.set_style(style);
}
