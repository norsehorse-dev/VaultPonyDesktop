//! Settings panel: auto-lock, theme, language, and defaults. Stub in P0; built
//! across P6 (auto-lock, privacy) and P7 (localization).

use eframe::egui::Ui;

use crate::app::App;

pub fn ui(_app: &mut App, ui: &mut Ui) {
    super::coming_soon(
        ui,
        "Settings",
        "P6 / P7",
        "Auto-lock timeout, theme, language, and default cipher and hash. \
         Auto-lock and privacy in P6; the six-language switch in P7.",
    );
}
