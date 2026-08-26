//! The macFUSE / FUSE backend (feature `fuse`).
//!
//! A thin `fuser::Filesystem` over [`FsCore`]: it owns the unlocked `Session`
//! and forwards each filesystem call to the tested translation layer. Read-only
//! for now, so no write, create, or unlink is implemented and the volume mounts
//! `MountOption::RO`. Nothing here is unit-tested (it needs a real kernel mount);
//! the logic it delegates to is.

use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{Context, Result};
use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    Request,
};
use vault_core::Session;

use super::fs_core::{Attr, FsCore};
use super::MountHandle;
use crate::vfsops;

const ENOENT: i32 = 2;
const EIO: i32 = 5;
const TTL: Duration = Duration::from_secs(1);

struct VaultFs {
    session: Session,
    fs: FsCore,
}

fn to_fileattr(a: &Attr, uid: u32, gid: u32) -> FileAttr {
    let mtime = a
        .mtime_ms
        .filter(|ms| *ms > 0)
        .map(|ms| UNIX_EPOCH + Duration::from_millis(ms as u64))
        .unwrap_or(UNIX_EPOCH);
    let (kind, perm, nlink) = if a.is_dir {
        (FileType::Directory, 0o555, 2)
    } else {
        (FileType::RegularFile, 0o444, 1)
    };
    FileAttr {
        ino: a.ino,
        size: a.size,
        blocks: a.size.div_ceil(512),
        atime: mtime,
        mtime,
        ctime: mtime,
        crtime: mtime,
        kind,
        perm,
        nlink,
        uid,
        gid,
        rdev: 0,
        blksize: 512,
        flags: 0,
    }
}

impl Filesystem for VaultFs {
    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let Some(name) = name.to_str() else {
            return reply.error(ENOENT);
        };
        match self.fs.lookup(self.session.vfs(), parent, name) {
            Ok(attr) => reply.entry(&TTL, &to_fileattr(&attr, req.uid(), req.gid()), 0),
            Err(_) => reply.error(ENOENT),
        }
    }

    fn getattr(&mut self, req: &Request, ino: u64, reply: ReplyAttr) {
        match self.fs.getattr(self.session.vfs(), ino) {
            Ok(attr) => reply.attr(&TTL, &to_fileattr(&attr, req.uid(), req.gid())),
            Err(_) => reply.error(ENOENT),
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock: Option<u64>,
        reply: ReplyData,
    ) {
        if offset < 0 {
            return reply.error(EIO);
        }
        match self
            .fs
            .read(self.session.vfs(), ino, offset as u64, size as usize)
        {
            Ok(data) => reply.data(&data),
            Err(_) => reply.error(EIO),
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let cur = match self.fs.path_of(ino) {
            Some(p) => p.to_owned(),
            None => return reply.error(ENOENT),
        };
        let parent_ino = self.fs.ino_for_path(vfsops::parent_of(&cur));
        let children = match self.fs.readdir(self.session.vfs(), ino) {
            Ok(c) => c,
            Err(_) => return reply.error(EIO),
        };

        let mut all: Vec<(u64, FileType, String)> = Vec::with_capacity(children.len() + 2);
        all.push((ino, FileType::Directory, ".".to_owned()));
        all.push((parent_ino, FileType::Directory, "..".to_owned()));
        for e in children {
            let kind = if e.is_dir {
                FileType::Directory
            } else {
                FileType::RegularFile
            };
            all.push((e.ino, kind, e.name));
        }

        for (i, (e_ino, kind, name)) in all.into_iter().enumerate().skip(offset as usize) {
            // The reply's offset is the index of the *next* entry to resume at.
            if reply.add(e_ino, (i + 1) as i64, kind, name) {
                break;
            }
        }
        reply.ok();
    }
}

/// A fresh, neutrally named mount point. The name is fixed and reveals nothing
/// about the container (threat model): not its filename, and never that it is a
/// hidden volume.
fn make_mountpoint() -> Result<PathBuf> {
    let base = std::env::temp_dir().join("VaultPony");
    std::fs::create_dir_all(&base)
        .with_context(|| format!("creating mount point {}", base.display()))?;
    Ok(base)
}

pub fn mount_readonly(session: Session) -> Result<MountHandle> {
    let mountpoint = make_mountpoint()?;
    let fs = VaultFs {
        session,
        fs: FsCore::new(),
    };
    let options = [
        MountOption::RO,
        MountOption::FSName("vaultpony".to_owned()),
        MountOption::DefaultPermissions,
    ];
    let bg = fuser::spawn_mount2(fs, &mountpoint, &options)
        .with_context(|| format!("mounting at {}", mountpoint.display()))?;
    Ok(MountHandle {
        mountpoint,
        _session: bg,
    })
}
