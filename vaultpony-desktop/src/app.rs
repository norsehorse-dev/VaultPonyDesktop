//! App state and the top-level UI: the tab rail and the per-tab panels.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui;
use egui::Context;
use vault_core::{ContainerFs, KdfFilter, Session};
use vc_fs::{DirEntry, FsKind};
use zeroize::Zeroize;

use crate::i18n::{self, Key};
use crate::{mark, mount, panels, settings, tasks, theme};

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

    pub fn key(self) -> Key {
        match self {
            Tab::Volumes => Key::TabVolumes,
            Tab::Files => Key::TabFiles,
            Tab::Create => Key::TabCreate,
            Tab::Tools => Key::TabTools,
            Tab::Settings => Key::TabSettings,
        }
    }

    pub fn icon(self) -> char {
        use crate::theme::ic;
        match self {
            Tab::Volumes => ic::LOCK,
            Tab::Files => ic::FILES,
            Tab::Create => ic::PLUS,
            Tab::Tools => ic::KEY_ROUND,
            Tab::Settings => ic::SETTINGS,
        }
    }
}

/// Light, dark, or follow-the-system theme (P6/overhaul).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeChoice {
    Light,
    Dark,
    Auto,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 3] = [ThemeChoice::Light, ThemeChoice::Dark, ThemeChoice::Auto];

    pub fn label(self) -> &'static str {
        match self {
            ThemeChoice::Light => "Light",
            ThemeChoice::Dark => "Dark",
            ThemeChoice::Auto => "Auto",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
            ThemeChoice::Auto => "auto",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "light" => ThemeChoice::Light,
            "dark" => ThemeChoice::Dark,
            _ => ThemeChoice::Auto,
        }
    }

    pub fn apply(self, ctx: &Context) {
        ctx.set_theme(match self {
            ThemeChoice::Light => egui::ThemePreference::Light,
            ThemeChoice::Dark => egui::ThemePreference::Dark,
            ThemeChoice::Auto => egui::ThemePreference::System,
        });
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
    /// Result of the last browser action, shown in the Files panel.
    pub op_status: String,
    /// Opened with hidden-volume write protection (the outer volume, P3).
    pub protected: bool,
}

/// Size unit for the create wizard.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SizeUnit {
    Mib,
    Gib,
}

impl SizeUnit {
    pub const ALL: [SizeUnit; 2] = [SizeUnit::Mib, SizeUnit::Gib];

    pub fn label(self) -> &'static str {
        match self {
            SizeUnit::Mib => "MiB",
            SizeUnit::Gib => "GiB",
        }
    }

    fn multiplier(self) -> u64 {
        match self {
            SizeUnit::Mib => 1024 * 1024,
            SizeUnit::Gib => 1024 * 1024 * 1024,
        }
    }

    /// Parse a size field into bytes, or `None` if it is not a positive number.
    pub fn to_bytes(self, value: &str) -> Option<u64> {
        let v: f64 = value.trim().parse().ok()?;
        if v <= 0.0 || !v.is_finite() {
            return None;
        }
        Some((v * self.multiplier() as f64) as u64)
    }
}

/// The create-wizard form state (Create panel).
pub struct CreateForm {
    pub out_path: Option<PathBuf>,
    pub size_value: String,
    pub size_unit: SizeUnit,
    pub scheme: String,
    pub prf: String,
    pub fs: ContainerFs,
    pub pim: String,
    pub pass: String,
    pub pass2: String,
    pub keyfiles: Vec<PathBuf>,
    pub status: String,

    // Hidden volume (P3). Reuses the outer cipher, hash, and filesystem.
    pub hidden: bool,
    pub hidden_pass: String,
    pub hidden_pass2: String,
    pub hidden_pim: String,
    pub hidden_size_value: String,
    pub hidden_size_unit: SizeUnit,
}

impl Default for CreateForm {
    fn default() -> Self {
        Self {
            out_path: None,
            size_value: "10".to_owned(),
            size_unit: SizeUnit::Mib,
            scheme: "AES".to_owned(),
            prf: "SHA-512".to_owned(),
            fs: ContainerFs::Fat,
            pim: String::new(),
            pass: String::new(),
            pass2: String::new(),
            keyfiles: Vec::new(),
            status: String::new(),
            hidden: false,
            hidden_pass: String::new(),
            hidden_pass2: String::new(),
            hidden_pim: String::new(),
            hidden_size_value: "2".to_owned(),
            hidden_size_unit: SizeUnit::Mib,
        }
    }
}

/// The header-tools form state (Tools panel, P4). `pass`/`pim` are the
/// container's current secret, reused for info, restore, and the old side of a
/// password change; `new_*` are the new secret for a change.
#[derive(Default)]
pub struct ToolsForm {
    pub pass: String,
    pub pim: String,
    pub new_pass: String,
    pub new_pass2: String,
    pub new_pim: String,
    /// Target KDF/hash for a password change, by registry name; `None` keeps
    /// the current one.
    pub new_kdf: Option<String>,
    pub output: String,
}

pub struct App {
    pub tab: Tab,

    // Unlock form state (Volumes panel).
    pub container_path: Option<PathBuf>,
    pub password: String,
    pub pim: String,
    // Hidden-protected outer unlock (P3): open outer read-write, protecting the
    // hidden volume. Needs the hidden password too.
    pub protect_hidden: bool,
    pub hidden_password: String,
    unlock_was_protected: bool,
    /// Which KDFs the unlock tries (Argon2id support, VeraCrypt 1.26.29+).
    /// Auto matches VeraCrypt; narrowing skips the other family's cost.
    pub unlock_kdf: KdfFilter,

    // In-flight unlock and its user-visible status line.
    pub job: Option<tasks::UnlockJob>,
    pub status: String,

    // The unlocked volume, if any. P0 holds one at a time; the mount table
    // (many at once) arrives with P5.
    pub volume: Option<Volume>,

    // Files-panel input state.
    pub new_dir_name: String,
    pub pending_delete: Option<String>,

    // Create-panel state.
    pub create: CreateForm,
    pub create_job: Option<tasks::CreateJob>,

    // Tools-panel state.
    pub tools: ToolsForm,
    pub tool_job: Option<tasks::ToolJob>,

    // Mount state (P5). When mounted, the session has moved into the mount, so
    // `volume` is None and `mounted_path` records what is mounted for display.
    pub mount: Option<mount::MountHandle>,
    pub mounted_path: Option<PathBuf>,

    // Settings and privacy (P6).
    pub settings: settings::Settings,
    /// Last time the user interacted, for the idle auto-lock.
    last_active: Instant,
    /// The app-icon texture for the rail, loaded once on first frame.
    icon_tex: Option<egui::TextureHandle>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = settings::Settings::load();
        ThemeChoice::from_code(&settings.theme).apply(&cc.egui_ctx);
        // Apply persisted preferences: the Create wizard's starting cipher/hash,
        // and the remembered container (unless no-trace suppresses it).
        let create = CreateForm {
            scheme: settings.default_scheme.clone(),
            prf: settings.default_prf.clone(),
            ..CreateForm::default()
        };
        let container_path = settings.remembered_container();

        Self {
            tab: Tab::Volumes,
            container_path,
            password: String::new(),
            pim: String::new(),
            protect_hidden: false,
            hidden_password: String::new(),
            unlock_was_protected: false,
            unlock_kdf: KdfFilter::Auto,
            job: None,
            status: String::new(),
            volume: None,
            new_dir_name: String::new(),
            pending_delete: None,
            create,
            create_job: None,
            tools: ToolsForm::default(),
            tool_job: None,
            mount: None,
            mounted_path: None,
            settings,
            last_active: Instant::now(),
            icon_tex: None,
        }
    }

    /// The current UI language, from settings.
    pub fn lang(&self) -> i18n::Lang {
        i18n::Lang::from_code(&self.settings.language)
    }

    /// The localized string for `key` in the current language.
    pub fn t(&self, key: Key) -> &'static str {
        i18n::t(self.lang(), key)
    }

    /// Translate a UI string keyed by its own English text, for the long tail of
    /// strings without a dedicated [`Key`]. Templates keep their `{placeholder}`
    /// tokens so the caller can `.replace()` them.
    pub fn tr(&self, en: &'static str) -> &'static str {
        i18n::tr(self.lang(), en)
    }

    /// Idle auto-lock and window-focus obscuring (P6). Returns true when the
    /// window contents should be hidden this frame (unfocused with a volume
    /// open and the obscure setting on), so the caller draws a cover instead.
    fn privacy_pass(&mut self, ctx: &Context) -> bool {
        let active = self.volume.is_some() || self.mount.is_some();

        // Any input this frame counts as activity.
        if ctx.input(|i| !i.events.is_empty()) {
            self.last_active = Instant::now();
        }

        if active {
            // Wake about once a second so the idle timer fires even with no input.
            ctx.request_repaint_after(Duration::from_secs(1));
            if settings::should_auto_lock(
                self.last_active.elapsed(),
                self.settings.auto_lock_minutes,
                true,
            ) {
                self.lock();
                self.status = self.tr("Auto-locked after inactivity.").to_owned();
                return false;
            }
        }

        let focused = ctx.input(|i| i.focused);
        self.settings.obscure_on_unfocus && !focused && active
    }

    /// Mount the open volume read-only as a real drive, moving its session into
    /// the mount. On failure the volume is gone (its session was consumed), so
    /// the user re-unlocks; that is stated in the status line.
    pub fn start_mount(&mut self) {
        let Some(volume) = self.volume.take() else {
            return;
        };
        let path = volume.path.clone();
        match mount::mount_readonly(volume.session) {
            Ok(handle) => {
                self.status = self
                    .tr("Mounted at {path}")
                    .replace("{path}", &handle.mountpoint().display().to_string());
                self.mounted_path = Some(path);
                self.mount = Some(handle);
            }
            Err(e) => {
                self.status = self
                    .tr("Mount failed: {e}. Unlock again to retry.")
                    .replace("{e}", &e.to_string());
            }
        }
    }

    /// Unmount and drop the mounted session (zeroizing keys).
    pub fn unmount(&mut self) {
        self.mount = None;
        self.mounted_path = None;
        self.status = self.tr("Unmounted.").to_owned();
    }

    /// Kick off an unlock on the worker thread from the current form state.
    pub fn start_unlock(&mut self, ctx: &Context) {
        let Some(path) = self.container_path.clone() else {
            self.status = self.tr("Choose a container file first.").to_owned();
            return;
        };
        let pim = self.pim.trim().parse::<u32>().unwrap_or(0);
        self.status = self.tr("Deriving keys...").to_owned();
        self.volume = None;
        self.unlock_was_protected = self.protect_hidden;

        if self.protect_hidden {
            // Hidden-protected outer unlock: outer password opens the outer
            // volume read-write, hidden password only marks the region to guard.
            let outer = vc_crypto::apply_keyfiles(self.password.as_bytes(), &[]);
            let hidden = vc_crypto::apply_keyfiles(self.hidden_password.as_bytes(), &[]);
            self.password.zeroize();
            self.hidden_password.zeroize();
            self.job = Some(tasks::spawn_unlock_protected(
                ctx.clone(),
                path,
                outer,
                hidden,
                pim,
                self.unlock_kdf,
            ));
        } else {
            // Fold keyfiles into the passphrase. None wired into the unlock form
            // yet. Open read-write so the browser's add, create, and delete work;
            // a read-only toggle can gate this later (groundwork for P5).
            let secret = vc_crypto::apply_keyfiles(self.password.as_bytes(), &[]);
            self.password.zeroize();
            self.job = Some(tasks::spawn_unlock(
                ctx.clone(),
                path,
                secret,
                pim,
                true,
                self.unlock_kdf,
            ));
        }
    }

    /// Ask the in-flight unlock to stop. It stops before its next key
    /// derivation and reports back as cancelled.
    pub fn cancel_unlock(&mut self) {
        if let Some(job) = &self.job {
            job.cancel();
            self.status = self.tr("Cancelling...").to_owned();
        }
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
                    self.status = self
                        .tr("Trying {prf} ({i}/{n})...")
                        .replace("{prf}", &prf)
                        .replace("{i}", &(i + 1).to_string())
                        .replace("{n}", &n.to_string());
                }
                tasks::UnlockMsg::Done(Ok(session)) => {
                    self.adopt_session(session);
                    finished = true;
                }
                tasks::UnlockMsg::Done(Err(e)) => {
                    self.status = self.tr("Could not unlock: {e}").replace("{e}", &e);
                    finished = true;
                }
                tasks::UnlockMsg::Cancelled => {
                    self.status = self.tr("Unlock cancelled.").to_owned();
                    finished = true;
                }
            }
        }
        if finished {
            self.job = None;
        }
    }

    /// Drain an in-flight create.
    fn poll_create(&mut self) {
        use std::sync::mpsc::TryRecvError;

        let done = match self.create_job.as_ref() {
            None => return,
            Some(job) => match job.rx.try_recv() {
                Ok(tasks::CreateMsg::Done(result)) => result,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => Err("the create worker stopped".to_owned()),
            },
        };
        self.create_job = None;

        match done {
            Ok(path) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.create.status = self.tr("Created {name}.").replace("{name}", &name);
                // Prefill the Volumes tab so it can be unlocked straight away.
                self.container_path = Some(path);
                self.status = self.tr("Ready to unlock {name}.").replace("{name}", &name);
                self.tab = Tab::Volumes;
            }
            Err(e) => self.create.status = self.tr("Could not create: {e}").replace("{e}", &e),
        }
    }

    /// Drain an in-flight header-tool operation.
    fn poll_tool(&mut self) {
        use std::sync::mpsc::TryRecvError;

        let result = match self.tool_job.as_ref() {
            None => return,
            Some(job) => match job.rx.try_recv() {
                Ok(r) => r,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => Err("the tool worker stopped".to_owned()),
            },
        };
        self.tool_job = None;
        self.tools.output = match result {
            Ok(text) => text,
            Err(e) => self.tr("Error: {e}").replace("{e}", &e),
        };
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
            op_status: String::new(),
            protected: self.unlock_was_protected,
        });
        self.new_dir_name.clear();
        self.pending_delete = None;
        self.refresh_listing();
        self.status = self
            .tr("Unlocked. {scheme} / {prf}.")
            .replace("{scheme}", scheme)
            .replace("{prf}", prf);
        // Land in the browser: unlocking is a means, browsing is the point.
        self.tab = Tab::Files;

        // Remember the container for next launch, unless no-trace is on.
        if !self.settings.no_trace && self.settings.last_container != self.container_path {
            self.settings.last_container = self.container_path.clone();
            let _ = self.settings.save();
        }
    }

    /// Re-list the current directory of the open volume.
    pub fn refresh_listing(&mut self) {
        let Some(v) = self.volume.as_mut() else {
            return;
        };
        match crate::vfsops::list_sorted(v.session.vfs(), &v.cwd) {
            Ok(entries) => {
                v.entries = entries;
                v.list_error = None;
            }
            Err(e) => {
                v.entries.clear();
                v.list_error = Some(e.to_string());
            }
        }
    }

    /// Lock and zeroize the open volume, unmounting first if it is mounted.
    pub fn lock(&mut self) {
        // Unmounting drops the mounted session, which zeroizes its keys.
        self.mount = None;
        self.mounted_path = None;
        if let Some(v) = self.volume.take() {
            v.session.lock();
        }
        self.status = self.tr("Locked.").to_owned();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_job();
        self.poll_create();
        self.poll_tool();

        let ctx = ui.ctx().clone();
        if self.privacy_pass(&ctx) {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.label(self.tr("Contents hidden while the window is not focused."));
                });
            });
            return;
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.horizontal_top(|ui| {
                // Left rail.
                ui.vertical(|ui| {
                    ui.set_width(172.0);
                    ui.add_space(theme::space::SM);
                    let tex = self.icon_tex.get_or_insert_with(|| {
                        ui.ctx().load_texture(
                            "app-icon",
                            mark::rail_image(44),
                            egui::TextureOptions::LINEAR,
                        )
                    });
                    ui.horizontal(|ui| {
                        ui.image((tex.id(), egui::vec2(22.0, 22.0)));
                        ui.add_space(theme::space::TIGHT);
                        ui.label(
                            egui::RichText::new("VaultPony")
                                .font(theme::semibold(17.0))
                                .color(theme::ink(ui)),
                        );
                    });
                    ui.add_space(theme::space::LG);
                    for tab in Tab::ALL {
                        let label = self.t(tab.key());
                        if theme::rail_item(ui, tab.icon(), label, self.tab == tab).clicked() {
                            self.tab = tab;
                        }
                        ui.add_space(theme::space::TIGHT);
                    }
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.add_space(theme::space::SM);
                        ui.label(
                            egui::RichText::new(vault_core::core_version())
                                .font(egui::FontId::proportional(10.5))
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                });

                ui.add_space(theme::space::MD);
                ui.separator();
                ui.add_space(theme::space::LG);

                // Content for the selected tab.
                ui.vertical(|ui| {
                    ui.set_max_width(620.0);
                    match self.tab {
                        Tab::Volumes => panels::volumes::ui(self, ui),
                        Tab::Files => panels::files::ui(self, ui),
                        Tab::Create => panels::create::ui(self, ui),
                        Tab::Tools => panels::tools::ui(self, ui),
                        Tab::Settings => panels::settings::ui(self, ui),
                    }
                });
            });
        });
    }
}
