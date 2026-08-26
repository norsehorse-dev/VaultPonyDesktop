//! Headless coverage of the mount translation layer (P5).
//!
//! Exercises `FsCore` (the Vfs-to-filesystem mapping the FUSE backend delegates
//! to) against a real container, with no kernel mount and no FUSE library. This
//! is the tested core of mounting; the platform backend is thin glue over it.

use std::fs;
use std::path::{Path, PathBuf};

use vault_core::{ContainerFs, Session};
use vaultpony_desktop::mount::fs_core::{FsCore, ROOT_INO};
use vaultpony_desktop::newvolume::{self, Spec};
use vaultpony_desktop::vfsops;
use zeroize::Zeroizing;

const PASS: &[u8] = b"mount test password";
const PIM: u32 = 1;

fn scratch(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}

fn create_fat(path: &Path) -> anyhow::Result<()> {
    newvolume::create(&Spec {
        path: path.to_path_buf(),
        size: 16 * 1024 * 1024,
        scheme: newvolume::scheme_by_name("AES").expect("AES"),
        prf: newvolume::prf_by_name("SHA-512").expect("SHA-512"),
        pim: PIM,
        fs: ContainerFs::Fat,
        passphrase: Zeroizing::new(PASS.to_vec()),
        keyfiles: Vec::new(),
        hidden: None,
    })
}

#[test]
fn fscore_maps_lookup_readdir_and_read() -> anyhow::Result<()> {
    // The mount always runs against a live unlocked session (it never reopens),
    // so build the tree and drive FsCore over that same session.
    let path = scratch("mount.hc");
    create_fat(&path)?;

    let payload: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
    let local = scratch("m_payload.bin");
    fs::write(&local, &payload)?;

    // add_file stores the file under its own local name.
    let stored_name = local.file_name().unwrap().to_str().unwrap().to_owned();
    let mut s = Session::unlock_with(&path, PASS, PIM, true, &mut |_, _, _| {})?;
    vfsops::mkdir(s.vfs(), "/", "docs")?;
    vfsops::add_file(s.vfs(), &local, "/docs")?;

    let mut fs = FsCore::new();
    let vfs = s.vfs();

    // readdir root lists the docs directory with a stable inode.
    let root = fs.readdir(vfs, ROOT_INO)?;
    let docs = root
        .iter()
        .find(|e| e.name == "docs")
        .expect("docs is listed");
    assert!(docs.is_dir);

    // lookup resolves the same inode readdir assigned.
    let docs_attr = fs.lookup(vfs, ROOT_INO, "docs")?;
    assert!(docs_attr.is_dir);
    assert_eq!(docs_attr.ino, docs.ino, "inode is stable across ops");

    // lookup the file inside, and confirm its size.
    let file_attr = fs.lookup(vfs, docs_attr.ino, &stored_name)?;
    assert!(!file_attr.is_dir);
    assert_eq!(file_attr.size, payload.len() as u64);

    // getattr agrees.
    let g = fs.getattr(vfs, file_attr.ino)?;
    assert_eq!(g.size, payload.len() as u64);
    assert_eq!(g.ino, file_attr.ino);

    // A full read returns the exact bytes.
    let whole = fs.read(vfs, file_attr.ino, 0, payload.len())?;
    assert_eq!(whole, payload, "full read matches");

    // A ranged read returns the right window.
    let part = fs.read(vfs, file_attr.ino, 10, 20)?;
    assert_eq!(part, &payload[10..30], "ranged read matches");

    // A read past end returns nothing.
    let past = fs.read(vfs, file_attr.ino, payload.len() as u64, 64)?;
    assert!(past.is_empty(), "read at EOF is empty");

    // An unknown inode is an error, not a panic.
    assert!(fs.getattr(vfs, 999_999).is_err());

    s.lock();
    Ok(())
}
