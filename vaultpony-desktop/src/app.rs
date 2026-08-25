//! App state and the top-level UI: the tab rail and the per-tab panels.

use std::path::PathBuf;

use eframe::egui;
use egui::Context;
use vault_core::Session;
use vc_fs::{DirEntry, FsKind};
use zeroize::Zeroize;

use crate::{panels, tasks};

/// The left-rail destinations. Panels are added as variants, not modes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Volumes,
    Files,
    Create,
    Tools,
    Settings,
}

impl Tab {
    pub const ALL: [Tab; 5] = [
        Tab::Volumes,
        Tab::Files,
        Tab::Create,
        Tab::Tools,
        Tab::Settings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Volumes => "Volumes",
            Tab::Files => "Files",
            Tab::Create => "Create",
            Tab::Tools => "Tools",
            Tab::Settings => "Settings",
        }
    }
}

/// One unlocked container. For P0 it carries just enough to prove the stack:
/// the session, what opened it, and a listing of the current directory.
pub struct Volume {
    pub path: PathBuf,
    pub scheme: &'static str,
    pub prf: &'static str,
    pub kind: FsKind,
    pub writable: bool,
    pub session: Session,
    pub cwd: String,
    pub entries: Vec<DirEntry>,
    pub list_error: Option<String>,
}

pub struct App {
    pub tab: Tab,

    // Unlock form state (Volumes panel).
    pub container_path: Option<PathBuf>,
    pub password: String,
    pub pim: String,

    // In-flight unlock and its user-visible status line.
    pub job: Option<tasks::UnlockJob>,
    pub status: String,

    // The unlocked volume, if any. P0 holds one at a time; the mount table
    // (many at once) arrives with P5.
    pub volume: Option<Volume>,
}

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            tab: Tab::Volumes,
            container_path: None,
            password: String::new(),
            pim: String::new(),
            job: None,
            status: String::new(),
            volume: None,
        }
    }

    /// Kick off an unlock on the worker thread from the current form state.
    pub fn start_unlock(&mut self, ctx: &Context) {
        let Some(path) = self.container_path.clone() else {
            self.status = "Choose a container file first.".to_owned();
            return;
        };
        let pim = self.pim.trim().parse::<u32>().unwrap_or(0);
        // Fold keyfiles into the passphrase. None in P0.
        let secret = vc_crypto::apply_keyfiles(self.password.as_bytes(), &[]);
        self.password.zeroize();
        self.status = "Deriving keys...".to_owned();
        self.volume = None;
        self.job = Some(tasks::spawn_unlock(ctx.clone(), path, secret, pim));
    }

    /// Drain any messages from an in-flight unlock. Messages are collected
    /// first so the channel borrow does not overlap the session-adopting calls.
    fn poll_job(&mut self) {
        use std::sync::mpsc::TryRecvError;

        let mut msgs = Vec::new();
        let mut finished = false;
        match self.job.as_ref() {
            None => return,
            Some(job) => loop {
                match job.rx.try_recv() {
                    Ok(msg) => msgs.push(msg),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        finished = true;
                        break;
                    }
                }
            },
        }

        for msg in msgs {
            match msg {
                tasks::UnlockMsg::Progress { i, n, prf } => {
                    self.status = format!("Trying {prf} ({}/{n})...", i + 1);
                }
                tasks::UnlockMsg::Done(Ok(session)) => {
                    self.adopt_session(session);
                    finished = true;
                }
                tasks::UnlockMsg::Done(Err(e)) => {
                    self.status = format!("Could not unlock: {e}");
                    finished = true;
                }
            }
        }
        if finished {
            self.job = None;
        }
    }

    /// Take a freshly unlocked session, record what opened it, and list root.
    fn adopt_session(&mut self, mut session: Session) {
        let scheme = session.scheme();
        let prf = session.prf();
        let kind = session.vfs().kind();
        let writable = session.vfs().writable();
        let path = self.container_path.clone().unwrap_or_default();
        self.volume = Some(Volume {
            path,
            scheme,
            prf,
            kind,
            writable,
            session,
            cwd: "/".to_owned(),
            entries: Vec::new(),
            list_error: None,
        });
        self.refresh_listing();
        self.status = format!("Unlocked. {scheme} / {prf}.");
        self.tab = Tab::Volumes;
    }

    /// Re-list the current directory of the open volume.
    pub fn refresh_listing(&mut self) {
        let Some(v) = self.volume.as_mut() else {
            return;
        };
        match v.session.vfs().list(&v.cwd) {
            Ok(mut entries) => {
                entries.sort_by(|a, b| {
                    (b.is_dir, a.name.to_lowercase()).cmp(&(a.is_dir, b.name.to_lowercase()))
                });
                v.entries = entries;
                v.list_error = None;
            }
            Err(e) => {
                v.entries.clear();
                v.list_error = Some(e.to_string());
            }
        }
    }

    /// Lock and zeroize the open volume.
    pub fn lock(&mut self) {
        if let Some(v) = self.volume.take() {
            v.session.lock();
        }
        self.status = "Locked.".to_owned();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_job();

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal_top(|ui| {
                // Left rail.
                ui.vertical(|ui| {
                    ui.set_width(140.0);
                    ui.add_space(6.0);
                    ui.heading("VaultPony");
                    ui.add_space(12.0);
                    for tab in Tab::ALL {
                        if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                            self.tab = tab;
                        }
                    }
                    ui.add_space(16.0);
                    ui.small(vault_core::core_version());
                });

                ui.separator();

                // Content for the selected tab.
                ui.vertical(|ui| match self.tab {
                    Tab::Volumes => panels::volumes::ui(self, ui),
                    Tab::Files => panels::files::ui(self, ui),
                    Tab::Create => panels::create::ui(self, ui),
                    Tab::Tools => panels::tools::ui(self, ui),
                    Tab::Settings => panels::settings::ui(self, ui),
                });
            });
        });
    }
}
