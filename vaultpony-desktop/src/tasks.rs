//! Off-thread work.
//!
//! Unlocking derives header keys through 500k PBKDF2 iterations, which must
//! never run on the UI thread. For P0 there is one job, unlock: it runs on a
//! spawned thread, streams progress back over a channel, and hands the finished
//! `Session` back the same way. Create and mount join this module in later
//! phases.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use egui::Context;
use vault_core::Session;
use zeroize::Zeroizing;

/// A message from an in-flight unlock.
pub enum UnlockMsg {
    /// Progress through the candidate PRF list: attempt `i` of `n`, named `prf`.
    Progress { i: usize, n: usize, prf: String },
    /// The unlock finished, with the session or a displayable error.
    Done(Result<Session, String>),
}

/// A running unlock. Poll `rx` each frame.
pub struct UnlockJob {
    pub rx: Receiver<UnlockMsg>,
}

/// Start an unlock on a worker thread. `secret` is the effective secret
/// (passphrase with any keyfiles already folded in); it moves into the thread
/// and is zeroized when the thread ends.
///
/// `writable` opens the container file read-write. It must match how the
/// browser treats the volume: opening read-only leaves the file descriptor
/// `O_RDONLY`, and any later write through the FAT adapter fails with `EBADF`
/// even though the adapter reports itself writable.
pub fn spawn_unlock(
    ctx: Context,
    path: PathBuf,
    secret: Zeroizing<Vec<u8>>,
    pim: u32,
    writable: bool,
) -> UnlockJob {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let progress_tx = tx.clone();
        let progress_ctx = ctx.clone();
        let mut on_progress = move |i: usize, n: usize, prf: &str| {
            let _ = progress_tx.send(UnlockMsg::Progress {
                i,
                n,
                prf: prf.to_string(),
            });
            progress_ctx.request_repaint();
        };
        let result = Session::unlock_with(&path, &secret, pim, writable, &mut on_progress);
        let _ = tx.send(UnlockMsg::Done(result.map_err(|e| e.to_string())));
        ctx.request_repaint();
    });
    UnlockJob { rx }
}

/// Start a hidden-protected outer unlock on a worker thread (P3). Opens the
/// outer volume read-write while using the hidden password only to learn the
/// region to protect; a write into that region is refused by the core. Both
/// secrets move into the thread and are zeroized when it ends.
pub fn spawn_unlock_protected(
    ctx: Context,
    path: PathBuf,
    outer: Zeroizing<Vec<u8>>,
    hidden: Zeroizing<Vec<u8>>,
    pim: u32,
) -> UnlockJob {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let progress_tx = tx.clone();
        let progress_ctx = ctx.clone();
        let mut on_progress = move |i: usize, n: usize, prf: &str| {
            let _ = progress_tx.send(UnlockMsg::Progress {
                i,
                n,
                prf: prf.to_string(),
            });
            progress_ctx.request_repaint();
        };
        let result = Session::unlock_outer_protected(&path, &outer, &hidden, pim, &mut on_progress);
        let _ = tx.send(UnlockMsg::Done(result.map_err(|e| e.to_string())));
        ctx.request_repaint();
    });
    UnlockJob { rx }
}

/// The result of an in-flight create.
pub enum CreateMsg {
    /// Done, with the created container's path or a displayable error.
    Done(Result<PathBuf, String>),
}

/// A running create. Poll `rx` each frame.
pub struct CreateJob {
    pub rx: Receiver<CreateMsg>,
}

/// Create a container on a worker thread. The `spec` moves into the thread,
/// carrying its passphrase, which is zeroized when the thread ends.
pub fn spawn_create(ctx: Context, spec: crate::newvolume::Spec) -> CreateJob {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = crate::newvolume::create(&spec)
            .map(|()| spec.path.clone())
            .map_err(|e| format!("{e:#}"));
        let _ = tx.send(CreateMsg::Done(result));
        ctx.request_repaint();
    });
    CreateJob { rx }
}

/// A running header-tool operation (P4). Its result is a displayable success
/// message (or the volume-info text) or a displayable error.
pub struct ToolJob {
    pub rx: Receiver<Result<String, String>>,
}

/// Run a header-tool operation on a worker thread. `f` owns everything it needs
/// (paths and secret copies), so nothing borrows the UI. It runs a key
/// derivation, which is why it does not run on the UI thread.
pub fn spawn_tool<F>(ctx: Context, f: F) -> ToolJob
where
    F: FnOnce() -> Result<String, String> + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(f());
        ctx.request_repaint();
    });
    ToolJob { rx }
}
