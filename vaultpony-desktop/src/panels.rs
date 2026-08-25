//! The panels. Each is a free function taking `&mut App` and `&mut Ui`, one per
//! left-rail destination. Volumes is real in P0; the rest are stubs that name
//! the phase they arrive in.

pub mod create;
pub mod files;
pub mod settings;
pub mod tools;
pub mod volumes;

use eframe::egui::Ui;

/// Shared stub body for panels that are not built yet.
pub(crate) fn coming_soon(ui: &mut Ui, title: &str, phase: &str, what: &str) {
    ui.add_space(8.0);
    ui.heading(title);
    ui.add_space(6.0);
    ui.label(format!("Arrives in {phase}."));
    ui.add_space(4.0);
    ui.label(what);
}
