//! Console-subsystem companion to the GUI binary. Exists so CLI verbs have
//! somewhere to print on Windows, where the GUI binary has no stdout. With no
//! verb it prints usage rather than opening a window.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    match vaultpony_desktop::cli::run(&vaultpony_desktop::args()) {
        Some(code) => ExitCode::from(code as u8),
        None => {
            eprintln!("vaultpony-cli: no verb given. Try --help.");
            ExitCode::from(2)
        }
    }
}
