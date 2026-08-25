//! Files panel: the in-app browser (navigate, extract, add, mkdir). Stub in P0;
//! built in P1, where it becomes the tested `Vfs` bridge the mount reuses.

use eframe::egui::Ui;

use crate::app::App;

pub fn ui(_app: &mut App, ui: &mut Ui) {
    super::coming_soon(
        ui,
        "Files",
        "P1",
        "Browse the open volume, extract files out, and add files in. P0 lists \
         the root directory on the Volumes panel to prove the stack end to end.",
    );
}
