//! Console-subsystem companion to the GUI binary. Exists so CLI verbs have
//! somewhere to print on Windows, where the GUI binary has no stdout. With no
//! verb it prints usage rather than opening a window.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    match vaultpony_desktop::cli::run(&vaultpony_desktop::args()) {
        Some(code) => ExitCode::from(code as u8),
        None => {
            // No verb: there is no window to open from the console binary, so
            // print what it is and how to get the verb list.
            println!("{}", vaultpony_desktop::cli::version_line());
            println!("\nRun `vaultpony-cli --help` for the verbs, or open VaultPony itself.");
            ExitCode::SUCCESS
        }
    }
}
