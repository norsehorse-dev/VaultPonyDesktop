//! Headless coverage of the create path (P2).
//!
//! Creates containers through the same `newvolume::create` the wizard drives,
//! then reopens them with the core, with no display. Covers a plain FAT volume
//! and an exFAT volume protected by a keyfile and a non-default PIM.

use std::fs;
use std::path::PathBuf;

use vault_core::{ContainerFs, Session};
use vaultpony_desktop::newvolume::{self, Spec};
use vaultpony_desktop::vfsops;
use zeroize::Zeroizing;

fn scratch(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}

fn spec(path: PathBuf, fs: ContainerFs, pim: u32, pass: &[u8], keyfiles: Vec<PathBuf>) -> Spec {
    Spec {
        path,
        size: 8 * 1024 * 1024,
        scheme: newvolume::scheme_by_name("AES").expect("AES"),
        prf: newvolume::prf_by_name("SHA-512").expect("SHA-512"),
        pim,
        fs,
        passphrase: Zeroizing::new(pass.to_vec()),
        keyfiles,
        hidden: None,
    }
}

#[test]
fn create_fat_reopens_and_lists_empty() -> anyhow::Result<()> {
    let path = scratch("create_fat.hc");
    newvolume::create(&spec(
        path.clone(),
        ContainerFs::Fat,
        0,
        b"open sesame please",
        vec![],
    ))?;

    let mut session =
        Session::unlock_with(&path, b"open sesame please", 0, false, &mut |_, _, _| {})?;
    assert_eq!(session.vfs().kind(), vc_fs::FsKind::Fat);
    assert!(vfsops::list_sorted(session.vfs(), "/")?.is_empty());
    session.lock();
    Ok(())
}

#[test]
fn create_exfat_with_keyfile_and_pim_reopens() -> anyhow::Result<()> {
    let path = scratch("create_exfat.hc");
    let keyfile = scratch("kf.bin");
    fs::write(&keyfile, b"a keyfile made of some bytes")?;
    let pim = 500;
    let pass: &[u8] = b"pass plus keyfile";

    newvolume::create(&spec(
        path.clone(),
        ContainerFs::Exfat,
        pim,
        pass,
        vec![keyfile.clone()],
    ))?;

    // The passphrase alone, without the keyfile, must not open it.
    assert!(
        Session::unlock_with(&path, pass, pim, false, &mut |_, _, _| {}).is_err(),
        "keyfile is required, so the bare passphrase fails"
    );

    // Fold the keyfile in at the same PIM and it opens.
    let kf = fs::read(&keyfile)?;
    let secret = vc_crypto::apply_keyfiles(pass, &[kf]);
    let mut session = Session::unlock_with(&path, &secret, pim, false, &mut |_, _, _| {})?;
    assert_eq!(session.vfs().kind(), vc_fs::FsKind::Exfat);
    assert!(vfsops::list_sorted(session.vfs(), "/")?.is_empty());
    session.lock();
    Ok(())
}
