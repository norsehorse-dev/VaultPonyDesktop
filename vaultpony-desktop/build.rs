//! Build script: derive the displayed version from a single source.
//!
//! The version shown in the app follows VaultPonyCore's workspace version (the
//! shared product version), so bumping it in one place updates the desktop
//! automatically. Falls back to this crate's own version if the core file is
//! not present (for example, a checkout without the submodule).

use std::fs;
use std::process::Command;

fn main() {
    let core_manifest = "../VaultPonyCore/Cargo.toml";
    println!("cargo:rerun-if-changed={core_manifest}");

    let version = fs::read_to_string(core_manifest)
        .ok()
        .and_then(|s| workspace_version(&s))
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());

    println!("cargo:rustc-env=VP_VERSION={version}");

    // The compiler that built this binary, so `vaultpony version` reports it
    // rather than the release workflow inferring it from the runner. CI asserts
    // that the shipped artifact on each platform reports a real rustc line.
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let rustc_version = Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "rustc (unknown)".to_owned());
    println!("cargo:rustc-env=VP_RUSTC={rustc_version}");
}

/// Pull `version = "X"` from the `[workspace.package]` table.
fn workspace_version(manifest: &str) -> Option<String> {
    let mut in_workspace_package = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_workspace_package = trimmed == "[workspace.package]";
            continue;
        }
        if in_workspace_package && trimmed.starts_with("version") {
            return trimmed.split('"').nth(1).map(str::to_owned);
        }
    }
    None
}
