//! The OS-mount layer (PLAN.md section 4).
//!
//! Mounting presents a decrypted volume to the OS as a real drive. The design
//! is a thin platform backend over a shared, tested translation ([`fs_core`]):
//! the backend speaks the platform's userspace-filesystem protocol, the
//! translation maps it onto the core's `Vfs`. Block-level mounting (what
//! VeraCrypt itself does) is ruled out: it needs signed kernel drivers on macOS
//! and Windows, and on Linux it means NBD, which collides with the zero-network
//! invariant.
//!
//! The macFUSE backend is behind the off-by-default `fuse` feature. macFUSE is a
//! non-free kernel extension and must never be bundled, so the shipped build
//! carries no FUSE dependency at all; a user opts in by installing macFUSE and
//! building with the feature. The in-app browser (P1) stays the default. This
//! module is written so `fs_core` and the whole UI compile and test with the
//! feature off; only the real backend and the `fuser` crate are gated.

pub mod fs_core;

#[cfg(feature = "fuse")]
mod fuse_backend;

use std::path::{Path, PathBuf};

use anyhow::Result;
use vault_core::Session;

/// Whether real OS mounting is possible here: the `fuse` feature is compiled in
/// and the FUSE runtime (macFUSE on macOS) is installed.
pub fn backend_available() -> bool {
    cfg!(feature = "fuse") && runtime_present()
}

/// A short, user-facing reason mounting is unavailable, or `None` when it is
/// available.
pub fn unavailable_reason() -> Option<&'static str> {
    if !cfg!(feature = "fuse") {
        return Some(
            "This build has no mount backend. Rebuild with `--features fuse` and install macFUSE to mount volumes as real drives.",
        );
    }
    if !runtime_present() {
        return Some(
            "The FUSE runtime is not installed. Install macFUSE from macfuse.github.io to mount volumes as real drives.",
        );
    }
    None
}

#[cfg(all(feature = "fuse", target_os = "macos"))]
fn runtime_present() -> bool {
    Path::new("/Library/Filesystems/macfuse.fs").exists()
}

#[cfg(all(feature = "fuse", target_os = "linux"))]
fn runtime_present() -> bool {
    Path::new("/dev/fuse").exists()
}

#[cfg(all(feature = "fuse", not(any(target_os = "macos", target_os = "linux"))))]
fn runtime_present() -> bool {
    false
}

#[cfg(not(feature = "fuse"))]
fn runtime_present() -> bool {
    false
}

/// A live mount. Dropping it unmounts the drive and drops the session, which
/// zeroizes the keys through the core's single lock path.
pub struct MountHandle {
    mountpoint: PathBuf,
    #[cfg(feature = "fuse")]
    _session: fuser::BackgroundSession,
}

impl MountHandle {
    pub fn mountpoint(&self) -> &Path {
        &self.mountpoint
    }
}

/// Mount `session`'s volume read-only at a fresh, neutrally named mount point.
/// The session moves into the filesystem loop and is owned there until unmount
/// (dropping the returned handle). The mount name reveals nothing about the
/// container, and never that it is a hidden volume (threat model).
#[cfg(feature = "fuse")]
pub fn mount_readonly(session: Session) -> Result<MountHandle> {
    fuse_backend::mount_readonly(session)
}

#[cfg(not(feature = "fuse"))]
pub fn mount_readonly(_session: Session) -> Result<MountHandle> {
    Err(anyhow::anyhow!(
        "this build has no mount backend (rebuild with --features fuse)"
    ))
}
