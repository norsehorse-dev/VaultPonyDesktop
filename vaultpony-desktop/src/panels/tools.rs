//! Tools panel: header backup and restore, change password, and a support-safe
//! volume info view over `probe`. Stub in P0; built in P4.

use eframe::egui::Ui;

use crate::app::App;

pub fn ui(_app: &mut App, ui: &mut Ui) {
    super::coming_soon(
        ui,
        "Tools",
        "P4",
        "Header backup and restore, change password and PIM, and a \
         support-safe volume info view (parameters only, never names or paths).",
    );
}
