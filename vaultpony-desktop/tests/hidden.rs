//! Headless coverage of hidden volumes (P3).
//!
//! Creates a container with a hidden volume, then proves the deniability
//! mechanics the desktop relies on: the outer and hidden passwords open
//! separate, isolated volumes by password alone, a wrong password opens
//! neither, and the protected outer unlock succeeds. The deeper write-protection
//! semantics (a write into the hidden region being refused) are covered by the
//! core's own `protect_gate`; here we confirm the desktop wiring reaches them.
//!
//! PIM 1 keeps the key derivations fast without changing the code paths.

use std::path::PathBuf;

use vault_core::{ContainerFs, Session};
use vaultpony_desktop::newvolume::{self, HiddenPart, Spec};
use vaultpony_desktop::vfsops;
use zeroize::Zeroizing;

const OUTER: &[u8] = b"outer volume secret";
const HIDDEN: &[u8] = b"hidden volume secret";
const PIM: u32 = 1;

fn scratch(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}

fn hidden_spec(path: PathBuf) -> Spec {
    Spec {
        path,
        size: 16 * 1024 * 1024,
        scheme: newvolume::scheme_by_name("AES").expect("AES"),
        prf: newvolume::prf_by_name("SHA-512").expect("SHA-512"),
        pim: PIM,
        fs: ContainerFs::Fat,
        passphrase: Zeroizing::new(OUTER.to_vec()),
        keyfiles: Vec::new(),
        hidden: Some(HiddenPart {
            passphrase: Zeroizing::new(HIDDEN.to_vec()),
            pim: PIM,
            keyfiles: Vec::new(),
            data_size: 4 * 1024 * 1024,
        }),
    }
}

fn has(entries: &[vc_fs::DirEntry], name: &str) -> bool {
    entries.iter().any(|e| e.name == name)
}

#[test]
fn hidden_and_outer_are_separate_volumes_by_password_alone() -> anyhow::Result<()> {
    let path = scratch("hidden.hc");
    newvolume::create(&hidden_spec(path.clone()))?;

    // The outer password opens the outer volume. Write a marker near its root
    // (far from the hidden tail region, so this is safe without protection).
    let mut outer = Session::unlock_with(&path, OUTER, PIM, true, &mut |_, _, _| {})?;
    vfsops::mkdir(outer.vfs(), "/", "outer_only")?;
    outer.lock();

    // The hidden password opens a different volume, with no "open hidden" flag.
    let mut hidden = Session::unlock_with(&path, HIDDEN, PIM, true, &mut |_, _, _| {})?;
    assert!(
        !has(&vfsops::list_sorted(hidden.vfs(), "/")?, "outer_only"),
        "the hidden volume does not see the outer volume's files"
    );
    vfsops::mkdir(hidden.vfs(), "/", "hidden_only")?;
    hidden.lock();

    // Reopening the outer volume shows its own file and never the hidden one.
    let mut outer2 = Session::unlock_with(&path, OUTER, PIM, false, &mut |_, _, _| {})?;
    let root = vfsops::list_sorted(outer2.vfs(), "/")?;
    assert!(has(&root, "outer_only"), "outer volume kept its own file");
    assert!(
        !has(&root, "hidden_only"),
        "outer volume cannot see the hidden volume's files"
    );
    outer2.lock();

    // A wrong password opens neither.
    assert!(
        Session::unlock_with(&path, b"neither of them", PIM, false, &mut |_, _, _| {}).is_err(),
        "a wrong password opens no volume"
    );
    Ok(())
}

#[test]
fn protected_outer_unlock_opens_read_write() -> anyhow::Result<()> {
    let path = scratch("hidden_protected.hc");
    newvolume::create(&hidden_spec(path.clone()))?;

    // Both passwords: outer opens the outer volume read-write, hidden marks the
    // region to guard. The open itself must succeed.
    let mut session =
        Session::unlock_outer_protected(&path, OUTER, HIDDEN, PIM, &mut |_, _, _| {})?;
    assert!(
        session.vfs().writable(),
        "protected outer volume is writable"
    );
    // A write in the outer's own area (its root) is allowed under protection.
    vfsops::mkdir(session.vfs(), "/", "safe_here")?;
    assert!(has(&vfsops::list_sorted(session.vfs(), "/")?, "safe_here"));
    session.lock();
    Ok(())
}
