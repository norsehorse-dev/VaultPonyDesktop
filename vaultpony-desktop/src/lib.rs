//! VaultPony Desktop.
//!
//! A library so the GUI binary (`vaultpony`) and the console binary
//! (`vaultpony-cli`) share one implementation. On macOS and Linux one binary
//! could do both jobs; on Windows a GUI-subsystem process has no stdout, so
//! `vaultpony-cli` exists as a second, console-subsystem binary over this code.

#![forbid(unsafe_code)]

pub mod app;
pub mod cli;
pub mod i18n;
pub mod mark;
pub mod mount;
pub mod newvolume;
pub mod panels;
pub mod settings;
pub mod tasks;
pub mod theme;
pub mod tools;
pub mod vfsops;

/// Open the window.
///
/// # Errors
///
/// Whatever `eframe` returns if the window cannot be created.
pub fn run_gui() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1020.0, 700.0])
            .with_min_inner_size([760.0, 500.0])
            .with_title("VaultPony")
            .with_icon(mark::window_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "VaultPony",
        options,
        Box::new(|cc| {
            theme::install(&cc.egui_ctx);
            Ok(Box::new(app::App::new(cc)))
        }),
    )
}

/// Arguments, minus the program name.
#[must_use]
pub fn args() -> Vec<String> {
    std::env::args().skip(1).collect()
}
