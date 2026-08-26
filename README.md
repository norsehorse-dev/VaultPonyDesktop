# VaultPonyDesktop

A desktop app for opening and editing VeraCrypt file containers, on Linux, macOS,
and Windows. Pure Rust, `egui`/`eframe`, backed by the shared Rust core
(VaultPonyCore) that also powers VaultPonyAndroid. No accounts, no telemetry, no
network code at all.

VeraCrypt is a registered trademark of IDRIX. VaultPony is not affiliated with or
endorsed by IDRIX. This is a clean-room implementation built on VaultPonyCore.

## Status

Early. This is the P0 walking skeleton: open a container, unlock it, and list its
root directory. The full roadmap, including the OS-level mount design, is in
`PLAN.md`.

## Building

The Rust core is a separate repo (VaultPonyCore), pulled in as a git submodule.
Clone with `--recursive`, or after cloning run:

    git submodule update --init --recursive

Then, with the toolchain pinned by `rust-toolchain.toml`:

    cargo run

`cargo run` launches the GUI. A second console binary exists for scripting and for
the Windows case where a GUI-subsystem process has no stdout:

    cargo run --bin vaultpony-cli -- --help

### Developing against a sibling core checkout

To build against `../VaultPonyCore` instead of the pinned submodule (for editing
core and desktop together), drop a `.cargo/config.toml` with a path override. That
file is gitignored and never affects a fresh clone.

### Mounting a volume as a real drive

Mounting is behind the off-by-default `fuse` feature. The shipped build carries no
FUSE dependency: macFUSE is a non-free kernel extension and is never bundled. To
mount volumes as real drives, install macFUSE from macfuse.github.io and build with
the feature:

    cargo run --features fuse

Without the feature (or without macFUSE installed), the Mount button is disabled
and the in-app browser on the Files tab is the way to work with volume contents.
Mounts are read-only for now.

## License

Apache-2.0. See `LICENSE`.
