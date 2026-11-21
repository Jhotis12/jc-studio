//! Visual tokens for the desktop client.
//!
//! One palette and one type scale for every panel. The app is an operator console, not a
//! landing page: calm surfaces, a single cool accent for action and focus, warm colors only
//! for warnings, and enough contrast to read at a glance.

use egui::{Color32, CornerRadius, FontId, Stroke, TextStyle};

/// Window and panel background.
pub const BG: Color32 = Color32::from_rgb(12, 15, 21);
/// Side panels, cards and the top bar.
pub const SURFACE: Color32 = Color32::from_rgb(19, 24, 32);
/// Raised cards and inputs.
pub const SURFACE_RAISED: Color32 = Color32::from_rgb(25, 31, 41);
/// Hover and selected-but-idle surfaces.
pub const SURFACE_HOVER: Color32 = Color32::from_rgb(31, 39, 51);
/// Sunken editors and terminals.
pub const SURFACE_SUNKEN: Color32 = Color32::from_rgb(9, 12, 17);
pub const BORDER: Color32 = Color32::from_rgb(39, 49, 63);
pub const BORDER_STRONG: Color32 = Color32::from_rgb(56, 70, 89);

pub const TEXT: Color32 = Color32::from_rgb(226, 235, 246);
pub const MUTED: Color32 = Color32::from_rgb(148, 163, 184);
pub const FAINT: Color32 = Color32::from_rgb(124, 139, 160);

/// Cool accent: actions, focus, live work.
pub const ACCENT: Color32 = Color32::from_rgb(88, 184, 255);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(26, 58, 86);
pub const SUCCESS: Color32 = Color32::from_rgb(96, 211, 146);
pub const WARNING: Color32 = Color32::from_rgb(242, 184, 102);
pub const DANGER: Color32 = Color32::from_rgb(255, 118, 118);
pub const NEUTRAL: Color32 = Color32::from_rgb(148, 163, 184);

pub const RADIUS_SM: CornerRadius = CornerRadius::same(6);
pub const RADIUS_MD: CornerRadius = CornerRadius::same(10);
pub const RADIUS_LG: CornerRadius = CornerRadius::same(14);

/// Applies the palette and type scale to a context.
pub fn apply(context: &egui::Context) {
    let mut visuals = egui::Visuals::dark();

    visuals.panel_fill = BG;
    visuals.window_fill = SURFACE;
    visuals.extreme_bg_color = SURFACE_SUNKEN;
    visuals.faint_bg_color = SURFACE;
    visuals.code_bg_color = SURFACE_SUNKEN;
    visuals.selection.bg_fill = ACCENT_SOFT;
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = WARNING;
    visuals.error_fg_color = DANGER;
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.window_corner_radius = RADIUS_LG;
    visuals.menu_corner_radius = RADIUS_MD;
    visuals.striped = true;

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = SURFACE;
    widgets.noninteractive.weak_bg_fill = SURFACE;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    widgets.noninteractive.corner_radius = RADIUS_SM;

    widgets.inactive.bg_fill = SURFACE_RAISED;
    widgets.inactive.weak_bg_fill = SURFACE_RAISED;
    widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    widgets.inactive.corner_radius = RADIUS_SM;

    widgets.hovered.bg_fill = SURFACE_HOVER;
    widgets.hovered.weak_bg_fill = SURFACE_HOVER;
    widgets.hovered.bg_stroke = Stroke::new(1.0_f32, BORDER_STRONG);
    widgets.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    widgets.hovered.corner_radius = RADIUS_SM;

    widgets.active.bg_fill = ACCENT_SOFT;
    widgets.active.weak_bg_fill = ACCENT_SOFT;
    widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    widgets.active.corner_radius = RADIUS_SM;

    widgets.open.bg_fill = SURFACE_HOVER;
    widgets.open.weak_bg_fill = SURFACE_HOVER;
    widgets.open.bg_stroke = Stroke::new(1.0_f32, BORDER_STRONG);
    widgets.open.fg_stroke = Stroke::new(1.0_f32, TEXT);
    widgets.open.corner_radius = RADIUS_SM;

    context.set_visuals(visuals);

    let mut style = (*context.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size = egui::vec2(40.0, 28.0);
    style.spacing.window_margin = egui::Margin::same(14);
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.indent = 18.0;
    style.text_styles = [
        (TextStyle::Heading, FontId::proportional(21.0)),
        (TextStyle::Body, FontId::proportional(14.0)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
        (TextStyle::Button, FontId::proportional(13.5)),
        (TextStyle::Small, FontId::proportional(11.5)),
    ]
    .into();
    context.set_style(style);
}

/// A regular card. The selected variant uses an accent border instead of a fill change so
/// the content stays readable and the state is obvious.
pub fn card(selected: bool) -> egui::Frame {
    egui::Frame::new()
        .fill(if selected { SURFACE_RAISED } else { SURFACE })
        .stroke(if selected {
            Stroke::new(1.5_f32, ACCENT)
        } else {
            Stroke::new(1.0_f32, BORDER)
        })
        .corner_radius(RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(13, 11))
}

/// A quieter surface for toolbars and nav rows.
pub fn bar() -> egui::Frame {
    egui::Frame::new()
        .fill(SURFACE)
        .corner_radius(RADIUS_MD)
        .inner_margin(egui::Margin::symmetric(8, 5))
}

/// A pill for status, cost and counters.
pub fn pill(fill: Color32) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(255))
        .inner_margin(egui::Margin::symmetric(8, 3))
}

pub fn muted(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).size(12.0).color(MUTED)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(color: Color32) -> f64 {
        let channels = [color.r(), color.g(), color.b()].map(|value| {
            let value = value as f64 / 255.0;
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]
    }

    fn contrast(left: Color32, right: Color32) -> f64 {
        let (light, dark) = if luminance(left) > luminance(right) {
            (luminance(left), luminance(right))
        } else {
            (luminance(right), luminance(left))
        };
        (light + 0.05) / (dark + 0.05)
    }

    #[test]
    fn secondary_text_meets_aa_on_both_surfaces() {
        for color in [TEXT, MUTED, FAINT] {
            assert!(
                contrast(color, BG) >= 4.5,
                "{color:?} on BG is {:.2}:1",
                contrast(color, BG)
            );
            assert!(
                contrast(color, SURFACE) >= 4.5,
                "{color:?} on SURFACE is {:.2}:1",
                contrast(color, SURFACE)
            );
        }
    }

    #[test]
    fn status_colors_are_readable() {
        for color in [SUCCESS, WARNING, DANGER, ACCENT] {
            assert!(
                contrast(color, SURFACE) >= 4.5,
                "{color:?} on SURFACE is {:.2}:1",
                contrast(color, SURFACE)
            );
        }
    }
}
