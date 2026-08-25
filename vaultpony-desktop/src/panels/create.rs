//! Create panel: the new-container wizard. Stub in P0; built in P2 (size,
//! cipher, hash, filesystem, PIM, keyfiles), with hidden volumes in P3.

use eframe::egui::Ui;

use crate::app::App;

pub fn ui(_app: &mut App, ui: &mut Ui) {
    super::coming_soon(
        ui,
        "Create",
        "P2",
        "Make a new VeraCrypt-compatible container: size, cipher, hash, \
         filesystem, PIM, and keyfiles. Hidden volumes land in P3.",
    );
}
