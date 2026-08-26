//! Build script: expose the app version and the compiler that built it.
//!
//! `VP_VERSION` is the desktop app's own version (the workspace package
//! version), the single number the deb, the MSI, and the macOS bundle also
//! carry. The shared core is versioned independently and reports itself
//! separately through `vault_core::core_version()`, so the two are not conflated.

use std::process::Command;

fn main() {
    // The desktop crate's version, inherited from the workspace package version.
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_owned());
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
