//! The Vfs-to-filesystem translation (P5), UI-free and backend-free so it is
//! tested headlessly.
//!
//! This is the heart of mounting: it maps the core's path-based `Vfs` onto the
//! inode-based operations a userspace filesystem (FUSE, WinFsp, FSKit) speaks.
//! The platform backend is a thin shell over these methods, so the same tested
//! logic drives every mount. Read-only for now: lookup, getattr, readdir, read.
//! Nothing here links a FUSE library.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use vc_fs::Vfs;

use crate::vfsops;

/// The root inode. FUSE fixes the mount root at inode 1.
pub const ROOT_INO: u64 = 1;

/// Displayable attributes for one node, backend-agnostic.
#[derive(Debug, Clone, Copy)]
pub struct Attr {
    pub ino: u64,
    pub is_dir: bool,
    pub size: u64,
    pub mtime_ms: Option<i64>,
}

/// One directory entry from `readdir`.
#[derive(Debug, Clone)]
pub struct Entry {
    pub ino: u64,
    pub name: String,
    pub is_dir: bool,
}

/// Maps stable inode numbers to volume paths and back. Inodes are assigned the
/// first time a path is seen and never reused within a mount, which is what a
/// userspace filesystem expects.
pub struct FsCore {
    by_ino: HashMap<u64, String>,
    by_path: HashMap<String, u64>,
    next: u64,
}

impl Default for FsCore {
    fn default() -> Self {
        Self::new()
    }
}

impl FsCore {
    pub fn new() -> Self {
        let mut by_ino = HashMap::new();
        let mut by_path = HashMap::new();
        by_ino.insert(ROOT_INO, "/".to_owned());
        by_path.insert("/".to_owned(), ROOT_INO);
        Self {
            by_ino,
            by_path,
            next: ROOT_INO + 1,
        }
    }

    /// The volume path for an inode, if known.
    pub fn path_of(&self, ino: u64) -> Option<&str> {
        self.by_ino.get(&ino).map(String::as_str)
    }

    /// The inode for a path, assigning one if new. A backend uses this for the
    /// `..` entry when listing a directory.
    pub fn ino_for_path(&mut self, path: String) -> u64 {
        self.intern(path)
    }

    /// Intern a path, returning its stable inode.
    fn intern(&mut self, path: String) -> u64 {
        if let Some(&ino) = self.by_path.get(&path) {
            return ino;
        }
        let ino = self.next;
        self.next += 1;
        self.by_ino.insert(ino, path.clone());
        self.by_path.insert(path, ino);
        ino
    }

    /// Resolve a child of `parent_ino` by name.
    pub fn lookup(&mut self, vfs: &mut dyn Vfs, parent_ino: u64, name: &str) -> Result<Attr> {
        let parent = self
            .path_of(parent_ino)
            .ok_or_else(|| anyhow!("unknown inode {parent_ino}"))?
            .to_owned();
        let path = vfsops::join(&parent, name);
        let st = vfs.stat(&path).map_err(|e| anyhow!(e.to_string()))?;
        let ino = self.intern(path);
        Ok(Attr {
            ino,
            is_dir: st.is_dir,
            size: st.size,
            mtime_ms: st.mtime_ms,
        })
    }

    /// Attributes of an inode.
    pub fn getattr(&mut self, vfs: &mut dyn Vfs, ino: u64) -> Result<Attr> {
        let path = self
            .path_of(ino)
            .ok_or_else(|| anyhow!("unknown inode {ino}"))?
            .to_owned();
        if ino == ROOT_INO {
            return Ok(Attr {
                ino: ROOT_INO,
                is_dir: true,
                size: 0,
                mtime_ms: None,
            });
        }
        let st = vfs.stat(&path).map_err(|e| anyhow!(e.to_string()))?;
        Ok(Attr {
            ino,
            is_dir: st.is_dir,
            size: st.size,
            mtime_ms: st.mtime_ms,
        })
    }

    /// List a directory inode, assigning inodes to its children.
    pub fn readdir(&mut self, vfs: &mut dyn Vfs, ino: u64) -> Result<Vec<Entry>> {
        let path = self
            .path_of(ino)
            .ok_or_else(|| anyhow!("unknown inode {ino}"))?
            .to_owned();
        let listing = vfsops::list_sorted(vfs, &path)?;
        let mut out = Vec::with_capacity(listing.len());
        for e in listing {
            let child = vfsops::join(&path, &e.name);
            let child_ino = self.intern(child);
            out.push(Entry {
                ino: child_ino,
                name: e.name,
                is_dir: e.is_dir,
            });
        }
        Ok(out)
    }

    /// Read up to `size` bytes at `offset` from a file inode. Returns the bytes
    /// actually available (shorter than `size` only at end of file).
    pub fn read(
        &mut self,
        vfs: &mut dyn Vfs,
        ino: u64,
        offset: u64,
        size: usize,
    ) -> Result<Vec<u8>> {
        let path = self
            .path_of(ino)
            .ok_or_else(|| anyhow!("unknown inode {ino}"))?
            .to_owned();
        let mut buf = vec![0u8; size];
        let mut filled = 0usize;
        while filled < size {
            let n = vfs
                .read_at(&path, offset + filled as u64, &mut buf[filled..])
                .map_err(|e| anyhow!(e.to_string()))?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        buf.truncate(filled);
        Ok(buf)
    }
}
