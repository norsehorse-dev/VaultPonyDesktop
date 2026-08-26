//! The VFS bridge: streaming, chunked file operations over the core's `Vfs`.
//!
//! This is the one place the file operations live, so the Files panel (P1) and
//! the OS mount (P5) share exactly the same read and write logic. It is
//! deliberately UI-free and synchronous, which is what lets it be tested
//! headlessly against a real container (see `tests/bridge.rs`).
//!
//! Reads and writes are chunked so neither an extract nor an add ever holds a
//! whole file in memory: plaintext lives in a bounded buffer and no longer.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use vc_fs::{DirEntry, Vfs};

/// Transfer chunk size.
pub const CHUNK: usize = 64 * 1024;

/// `/`-join a directory and a child name, matching the VFS path convention.
pub fn join(dir: &str, name: &str) -> String {
    if dir == "/" {
        format!("/{name}")
    } else {
        format!("{}/{name}", dir.trim_end_matches('/'))
    }
}

/// The parent directory of a VFS path, bottoming out at root.
pub fn parent_of(path: &str) -> String {
    match path.rfind('/') {
        Some(0) | None => "/".to_owned(),
        Some(i) => path[..i].to_owned(),
    }
}

/// The final path component.
pub fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Human-readable byte size.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

/// List a directory, directories first, then case-insensitive by name.
pub fn list_sorted(vfs: &mut dyn Vfs, path: &str) -> Result<Vec<DirEntry>> {
    let mut entries = vfs.list(path).map_err(vc)?;
    entries
        .sort_by(|a, b| (b.is_dir, a.name.to_lowercase()).cmp(&(a.is_dir, b.name.to_lowercase())));
    Ok(entries)
}

/// Copy one file out of the volume to a local path, streaming. Returns bytes.
pub fn extract_file(vfs: &mut dyn Vfs, src: &str, dest: &Path) -> Result<u64> {
    let mut out = File::create(dest).with_context(|| format!("creating {}", dest.display()))?;
    let mut buf = vec![0u8; CHUNK];
    let mut offset = 0u64;
    loop {
        let n = vfs.read_at(src, offset, &mut buf).map_err(vc)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        offset += n as u64;
    }
    out.flush()?;
    Ok(offset)
}

/// Copy a node (a file, or a whole subtree) out of the volume into `into_dir`,
/// keeping its name. Returns the number of files written.
pub fn extract(vfs: &mut dyn Vfs, src: &str, into_dir: &Path) -> Result<u64> {
    let target = into_dir.join(basename(src));
    let st = vfs.stat(src).map_err(vc)?;
    if st.is_dir {
        std::fs::create_dir_all(&target)?;
        let mut count = 0;
        for e in list_sorted(vfs, src)? {
            count += extract(vfs, &join(src, &e.name), &target)?;
        }
        Ok(count)
    } else {
        extract_file(vfs, src, &target)?;
        Ok(1)
    }
}

/// Copy a local file into the volume under `dest_dir`, streaming. The file is
/// stored under its own name. Returns bytes written.
pub fn add_file(vfs: &mut dyn Vfs, local: &Path, dest_dir: &str) -> Result<u64> {
    if !vfs.writable() {
        return Err(anyhow!("this volume is read-only"));
    }
    let name = local
        .file_name()
        .ok_or_else(|| anyhow!("{} has no file name", local.display()))?
        .to_string_lossy()
        .into_owned();
    let dest = join(dest_dir, &name);
    let mut input = File::open(local).with_context(|| format!("opening {}", local.display()))?;

    vfs.create(&dest).map_err(vc)?;
    let mut buf = vec![0u8; CHUNK];
    let mut offset = 0u64;
    loop {
        let n = input.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let mut written = 0;
        while written < n {
            let w = vfs
                .write_at(&dest, offset + written as u64, &buf[written..n])
                .map_err(vc)?;
            if w == 0 {
                return Err(anyhow!(
                    "write stalled at offset {}",
                    offset + written as u64
                ));
            }
            written += w;
        }
        offset += n as u64;
    }
    vfs.flush().map_err(vc)?;
    Ok(offset)
}

/// Make a directory named `name` under `parent`.
pub fn mkdir(vfs: &mut dyn Vfs, parent: &str, name: &str) -> Result<()> {
    if !vfs.writable() {
        return Err(anyhow!("this volume is read-only"));
    }
    if name.is_empty() || name.contains('/') {
        return Err(anyhow!("invalid directory name"));
    }
    vfs.mkdir(&join(parent, name)).map_err(vc)?;
    vfs.flush().map_err(vc)?;
    Ok(())
}

/// Delete a file at `path`. Directories are out of scope for P1.
pub fn delete_file(vfs: &mut dyn Vfs, path: &str) -> Result<()> {
    if !vfs.writable() {
        return Err(anyhow!("this volume is read-only"));
    }
    vfs.unlink(path).map_err(vc)?;
    vfs.flush().map_err(vc)?;
    Ok(())
}

/// Render a `VcError` into an `anyhow::Error` by its message. The bridge does
/// not branch on error kind, so the displayable string is all it needs.
fn vc(e: vc_types::VcError) -> anyhow::Error {
    anyhow!(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{basename, human_size, join, parent_of};

    #[test]
    fn join_from_root_and_deeper() {
        assert_eq!(join("/", "docs"), "/docs");
        assert_eq!(join("/docs", "taxes"), "/docs/taxes");
        assert_eq!(join("/docs/", "taxes"), "/docs/taxes");
    }

    #[test]
    fn parent_walks_up_to_root() {
        assert_eq!(parent_of("/docs/taxes"), "/docs");
        assert_eq!(parent_of("/docs"), "/");
        assert_eq!(parent_of("/"), "/");
    }

    #[test]
    fn basename_is_last_component() {
        assert_eq!(basename("/docs/taxes/w2.pdf"), "w2.pdf");
        assert_eq!(basename("/docs"), "docs");
        assert_eq!(basename("/"), "");
    }

    #[test]
    fn sizes_are_human() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1024), "1.0 KiB");
        assert_eq!(human_size(1536), "1.5 KiB");
        assert_eq!(human_size(1024 * 1024), "1.0 MiB");
    }
}
