//! Headless round-trip of the VFS bridge (P1) against a real container.
//!
//! Creates a FAT container with the core, unlocks it read-write, and drives
//! add / list / extract / mkdir / delete through `vfsops`, with no display and
//! no OS mount. This is the same code path the Files panel calls and the same
//! bridge the P5 mount will wrap, so keeping it green here proves the mechanism
//! before a kernel boundary is ever involved.

use std::fs;
use std::path::PathBuf;

use vault_core::{create_container, ContainerFs, Session};
use vaultpony_desktop::vfsops;
use vc_format::CreateParams;
use vc_io::FileDevice;

const PASSPHRASE: &[u8] = b"correct horse battery staple";

fn scratch(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}

fn scheme(name: &str) -> &'static vc_types::EncryptionScheme {
    vc_types::registry::ENCRYPTION_SCHEMES
        .iter()
        .find(|s| s.name == name)
        .expect("scheme in registry")
}

fn prf(name: &str) -> &'static vc_types::Prf {
    vc_types::registry::PRFS
        .iter()
        .find(|p| p.name == name)
        .expect("prf in registry")
}

/// Make a fresh, empty, FAT-formatted container on disk and return its path.
fn make_container(file: &str) -> PathBuf {
    let path = scratch(file);
    let size = 16 * 1024 * 1024u64;

    let f = fs::File::create(&path).unwrap();
    f.set_len(size).unwrap();
    drop(f);

    let params = CreateParams {
        scheme: scheme("AES"),
        prf: prf("SHA-512"),
        pim: 0,
        passphrase: PASSPHRASE,
        keyfiles: &[],
        size,
        sector_size: 512,
    };
    let dev = FileDevice::open_rw(&path).unwrap();
    create_container(Box::new(dev), &params, ContainerFs::Fat).unwrap();
    path
}

#[test]
fn add_list_extract_mkdir_delete_round_trip() -> anyhow::Result<()> {
    let container = make_container("bridge.hc");
    let mut session = Session::unlock_with(&container, PASSPHRASE, 0, true, &mut |_, _, _| {})?;
    let vfs = session.vfs();

    assert!(vfs.writable(), "a freshly formatted FAT volume is writable");
    assert!(
        vfsops::list_sorted(vfs, "/")?.is_empty(),
        "a new volume starts empty"
    );

    // mkdir
    vfsops::mkdir(vfs, "/", "docs")?;
    let root = vfsops::list_sorted(vfs, "/")?;
    assert_eq!(root.len(), 1);
    assert_eq!(root[0].name, "docs");
    assert!(root[0].is_dir);

    // add a local file spanning more than one transfer chunk
    let payload: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    let local = scratch("payload.bin");
    fs::write(&local, &payload)?;
    let written = vfsops::add_file(vfs, &local, "/docs")?;
    assert_eq!(written, payload.len() as u64);

    // it shows up with the right size
    let listing = vfsops::list_sorted(vfs, "/docs")?;
    assert_eq!(listing.len(), 1);
    assert_eq!(listing[0].name, "payload.bin");
    assert!(!listing[0].is_dir);
    assert_eq!(listing[0].size, payload.len() as u64);

    // extract it back out and confirm the bytes survived the round trip
    let out_dir = scratch("out");
    fs::create_dir_all(&out_dir)?;
    let files = vfsops::extract(vfs, "/docs/payload.bin", &out_dir)?;
    assert_eq!(files, 1);
    let recovered = fs::read(out_dir.join("payload.bin"))?;
    assert_eq!(recovered, payload, "extracted bytes match the original");

    // delete it
    vfsops::delete_file(vfs, "/docs/payload.bin")?;
    assert!(
        vfsops::list_sorted(vfs, "/docs")?.is_empty(),
        "the file is gone after delete"
    );

    session.lock();
    Ok(())
}

#[test]
fn extract_recurses_a_whole_subtree() -> anyhow::Result<()> {
    let container = make_container("bridge_tree.hc");
    let mut session = Session::unlock_with(&container, PASSPHRASE, 0, true, &mut |_, _, _| {})?;
    let vfs = session.vfs();

    vfsops::mkdir(vfs, "/", "tree")?;
    vfsops::mkdir(vfs, "/tree", "inner")?;

    let a = scratch("a.txt");
    let b = scratch("b.txt");
    fs::write(&a, b"top level")?;
    fs::write(&b, b"nested")?;
    vfsops::add_file(vfs, &a, "/tree")?;
    vfsops::add_file(vfs, &b, "/tree/inner")?;

    let out = scratch("tree_out");
    fs::create_dir_all(&out)?;
    let count = vfsops::extract(vfs, "/tree", &out)?;
    assert_eq!(count, 2, "both files came out");

    assert_eq!(fs::read(out.join("tree/a.txt"))?, b"top level");
    assert_eq!(fs::read(out.join("tree/inner/b.txt"))?, b"nested");

    session.lock();
    Ok(())
}

#[test]
fn read_only_unlock_refuses_writes_cleanly() -> anyhow::Result<()> {
    let container = make_container("bridge_ro.hc");
    // Open read-only: the file descriptor is O_RDONLY.
    let mut session = Session::unlock_with(&container, PASSPHRASE, 0, false, &mut |_, _, _| {})?;
    let vfs = session.vfs();

    // Reads still work.
    assert!(vfsops::list_sorted(vfs, "/")?.is_empty());

    // A write through a read-only descriptor must come back as a clean error,
    // never a panic. This is the failure the GUI hit by opening read-only while
    // still offering writes; the app now opens read-write for that reason.
    assert!(vfsops::mkdir(vfs, "/", "nope").is_err());

    session.lock();
    Ok(())
}
