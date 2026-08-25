//! VaultPony Desktop, the GUI binary.
//!
//! With no verb it opens the window. With a verb it defers to the shared CLI,
//! which is what lets CI exercise a packaged build without a display. On Windows
//! this binary is GUI-subsystem and has no console, which is why `vaultpony-cli`
//! exists as a separate console binary over the same code.

#![forbid(unsafe_code)]
// Do not pop a console window alongside the GUI on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    if let Some(code) = vaultpony_desktop::cli::run(&vaultpony_desktop::args()) {
        std::process::exit(code);
    }
    vaultpony_desktop::run_gui()
}
