//! The shared command line.
//!
//! For P0 this is deliberately thin: `--version` and `--help`, and nothing else
//! yet. The full verb surface (info, list, extract, add, mount, unmount) lands
//! in P8; the headless `vaultpony` CLI in VaultPonyCore already covers the
//! non-mount verbs in the meantime.
//!
//! Returns `Some(exit_code)` when it handled the arguments (the caller should
//! exit), or `None` when there was no verb and the GUI should open.

pub fn run(args: &[String]) -> Option<i32> {
    match args.first().map(String::as_str) {
        None => None,
        Some("--version" | "-V") => {
            println!(
                "vaultpony {} ({})",
                env!("CARGO_PKG_VERSION"),
                vault_core::core_version()
            );
            Some(0)
        }
        Some("--help" | "-h") => {
            print_help();
            Some(0)
        }
        Some(other) => {
            eprintln!("vaultpony: unknown argument '{other}'. Try --help.");
            Some(2)
        }
    }
}

fn print_help() {
    println!(
        "\
vaultpony {ver}

USAGE:
    vaultpony                 Open the VaultPony window.
    vaultpony --version       Print version and core version.
    vaultpony --help          Print this help.

The scriptable verbs (info, list, extract, add, mount, unmount) are planned for
P8. Until then, the headless CLI in VaultPonyCore covers the non-mount verbs.",
        ver = env!("CARGO_PKG_VERSION"),
    );
}
