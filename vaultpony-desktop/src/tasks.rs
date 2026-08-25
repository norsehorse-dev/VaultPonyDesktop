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
pub fn spawn_unlock(
    ctx: Context,
    path: PathBuf,
    secret: Zeroizing<Vec<u8>>,
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
        let result = Session::unlock_with(&path, &secret, pim, false, &mut on_progress);
        let _ = tx.send(UnlockMsg::Done(result.map_err(|e| e.to_string())));
        ctx.request_repaint();
    });
    UnlockJob { rx }
}
