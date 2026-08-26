//! The shared command line.
//!
//! Thin by design: the GUI is the product, and these verbs exist mostly so a
//! packaged build can be smoke-tested with no display. `version` reports the
//! product, core, and compiler; `selftest` exercises the real create/unlock/
//! read path in the shipped binary; `--help` lists them. The scriptable file
//! verbs (info, list, extract, add, mount, unmount) still live in the headless
//! `vaultpony` CLI inside VaultPonyCore.
//!
//! Returns `Some(exit_code)` when it handled the arguments (the caller should
//! exit), or `None` when there was no verb and the GUI should open.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use vault_core::{ContainerFs, Session};
use zeroize::Zeroizing;

use crate::newvolume::{self, Spec};

/// One line naming the product, the shared core, and the compiler. CI asserts
/// this prints, mentions `rustc`, and names the build architecture.
#[must_use]
pub fn version_line() -> String {
    format!(
        "VaultPony Desktop {} (core {})\n{}, {}",
        env!("VP_VERSION"),
        vault_core::core_version(),
        env!("VP_RUSTC"),
        std::env::consts::ARCH,
    )
}

pub fn run(args: &[String]) -> Option<i32> {
    match args.first().map(String::as_str) {
        None => None,
        Some("--version" | "-V" | "version") => {
            println!("{}", version_line());
            Some(0)
        }
        Some("--help" | "-h" | "help") => {
            print_help();
            Some(0)
        }
        Some("selftest") => Some(selftest()),
        Some(other) => {
            eprintln!("vaultpony: unknown argument '{other}'. Try --help.");
            Some(2)
        }
    }
}

fn print_help() {
    println!(
        "\
VaultPony Desktop {ver}

USAGE:
    vaultpony                 Open the VaultPony window.
    vaultpony version         Print the product, core, and compiler versions.
    vaultpony selftest        Create, unlock, and read a throwaway container.
    vaultpony --help          Print this help.

The scriptable file verbs (info, list, extract, add, mount, unmount) are served
by the headless CLI in VaultPonyCore.",
        ver = env!("VP_VERSION"),
    );
}

/// Create a throwaway container, unlock it read-write, round-trip a file through
/// it, and lock it. This exercises PBKDF2 key derivation, the VeraCrypt header
/// write and read, and the FAT filesystem bridge in this exact build, which is
/// the check no test suite running only on the build machine can make about a
/// shipped artifact. Prints `PASS - all N checks` on success, `FAIL - <what>`
/// on the first failure, and returns 0 or 1 accordingly.
fn selftest() -> i32 {
    match run_selftest() {
        Ok(n) => {
            println!("PASS - all {n} checks");
            0
        }
        Err(e) => {
            println!("FAIL - {e}");
            1
        }
    }
}

fn run_selftest() -> Result<u32, String> {
    // PIM 1 keeps derivation fast enough for a smoke test while still exercising
    // the real PBKDF2 path.
    const PASS: &[u8] = b"vaultpony-selftest-passphrase";
    const PIM: u32 = 1;
    const PAYLOAD: &[u8] = b"VaultPony self-test payload: round-trips through a real container.";

    let mut checks = 0u32;
    let path = scratch_path();
    // Clean up the throwaway container however we leave this function.
    let _guard = RemoveOnDrop(path.clone());

    // 1. Create a container through the same path the wizard drives.
    let scheme = newvolume::scheme_by_name("AES").ok_or("cipher AES not in registry")?;
    let prf = newvolume::prf_by_name("SHA-512").ok_or("hash SHA-512 not in registry")?;
    let spec = Spec {
        path: path.clone(),
        size: 1024 * 1024,
        scheme,
        prf,
        pim: PIM,
        fs: ContainerFs::Fat,
        passphrase: Zeroizing::new(PASS.to_vec()),
        keyfiles: Vec::new(),
        hidden: None,
    };
    newvolume::create(&spec).map_err(|e| format!("create failed: {e}"))?;
    checks += 1;

    // 2. Unlock it read-write with the same secret and PIM.
    let mut session = Session::unlock_with(&path, PASS, PIM, true, &mut |_, _, _| {})
        .map_err(|e| format!("unlock failed: {e}"))?;
    checks += 1;

    // 3. It reports the cipher and hash it was made with, and opened writable.
    if session.scheme().is_empty() || session.prf().is_empty() {
        return Err("unlocked session reported no cipher or hash".to_owned());
    }
    if session.vfs().kind() != vc_fs::FsKind::Fat {
        return Err("unlocked filesystem was not FAT".to_owned());
    }
    if !session.vfs().writable() {
        return Err("container opened read-only when read-write was requested".to_owned());
    }
    checks += 1;

    // 4. Write a file into it and flush.
    let vfs = session.vfs();
    let name = "/selftest.bin";
    vfs.create(name).map_err(|e| format!("create file: {e}"))?;
    let mut written = 0usize;
    while written < PAYLOAD.len() {
        let w = vfs
            .write_at(name, written as u64, &PAYLOAD[written..])
            .map_err(|e| format!("write: {e}"))?;
        if w == 0 {
            return Err("write stalled".to_owned());
        }
        written += w;
    }
    vfs.flush().map_err(|e| format!("flush: {e}"))?;
    checks += 1;

    // 5. Read it back and confirm the bytes survived the round trip.
    let mut buf = vec![0u8; PAYLOAD.len()];
    let mut read = 0usize;
    while read < buf.len() {
        let n = vfs
            .read_at(name, read as u64, &mut buf[read..])
            .map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        read += n;
    }
    if buf != PAYLOAD {
        return Err("round-tripped file did not match what was written".to_owned());
    }
    checks += 1;

    session.lock();
    Ok(checks)
}

/// A unique scratch path in the system temp directory. No `rand` dependency: the
/// process id and a nanosecond clock avoid a collision, and a stale file would
/// be overwritten anyway.
fn scratch_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("vaultpony-selftest-{pid}-{nanos}.hc"))
}

/// Deletes its path when dropped, so the self-test leaves nothing behind.
struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
