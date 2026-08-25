//! Visual theme.
//!
//! A compact dark theme for P0: spacing, corner radius, and an accent color, set
//! on the egui context at startup. AgePony's full theme (custom fonts, light and
//! dark palettes, the whole widget pass) is a later port; this is enough to make
//! the skeleton look deliberate rather than default-gray.

use egui::{Color32, Context, CornerRadius, Stroke, Visuals};

/// The VaultPony accent. A muted steel blue, readable on the dark ground.
pub const ACCENT: Color32 = Color32::from_rgb(0x5b, 0x8d, 0xef);

pub fn install(ctx: &Context) {
    let mut visuals = Visuals::dark();

    visuals.panel_fill = Color32::from_rgb(0x16, 0x18, 0x1d);
    visuals.window_fill = Color32::from_rgb(0x1b, 0x1e, 0x24);
    visuals.extreme_bg_color = Color32::from_rgb(0x0f, 0x11, 0x15);
    visuals.hyperlink_color = ACCENT;
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);

    let radius = CornerRadius::same(6);
    visuals.widgets.noninteractive.corner_radius = radius;
    visuals.widgets.inactive.corner_radius = radius;
    visuals.widgets.hovered.corner_radius = radius;
    visuals.widgets.active.corner_radius = radius;
    visuals.widgets.open.corner_radius = radius;

    ctx.set_visuals(visuals);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style.spacing.window_margin = egui::Margin::same(12);
    });
}
