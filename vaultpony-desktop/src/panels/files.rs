//! Files panel: the in-app browser.
//!
//! Navigate the open volume, extract files and subtrees out, add local files
//! in, make directories, and delete files. Every operation goes through
//! `crate::vfsops`, the same UI-free bridge the P5 mount will wrap, so what you
//! do here and what a mounted drive does run the identical code.
//!
//! Writes are guarded by `Vfs::writable`, so a read-only volume (NTFS today, or
//! exFAT until the core's write support lands) shows its files but refuses to
//! change them.

use std::path::PathBuf;

use eframe::egui::{self, Ui};

use crate::app::App;
use crate::i18n::Key;
use crate::theme::{self, ic};
use crate::vfsops;

/// A user action, collected during rendering and applied afterward so the
/// mutating calls do not fight the immutable borrow the listing holds.
enum Action {
    Navigate(String),
    Up,
    AddFile(PathBuf),
    Mkdir,
    Extract { src: String, into: PathBuf },
    AskDelete(String),
    DoDelete(String),
    CancelDelete,
}

/// Localized button labels, captured before the panel borrows `app` mutably.
struct Labels {
    up: &'static str,
    add: &'static str,
    new_folder: &'static str,
    new_folder_hint: &'static str,
    extract: &'static str,
    delete: &'static str,
    cancel: &'static str,
    read_only: &'static str,
    empty: &'static str,
    cannot_list: &'static str,
}

pub fn ui(app: &mut App, ui: &mut Ui) {
    theme::screen_head(ui, app.t(Key::TabFiles), app.t(Key::SubFiles), |_ui| {});

    if app.volume.is_none() {
        theme::empty_state(
            ui,
            ic::FILES,
            app.tr("Open a volume on the Volumes tab to browse its files."),
        );
        return;
    }

    // Localized labels captured before the mutable destructure below.
    let l = Labels {
        up: app.t(Key::Up),
        add: app.t(Key::AddFile),
        new_folder: app.t(Key::NewFolder),
        new_folder_hint: app.tr("new folder"),
        extract: app.t(Key::Extract),
        delete: app.t(Key::Delete),
        cancel: app.t(Key::Cancel),
        read_only: app.t(Key::HintReadOnly),
        empty: app.t(Key::EmptyDir),
        cannot_list: app.tr("Cannot list: {err}"),
    };

    let mut action: Option<Action> = None;

    theme::card(ui, |ui| {
        let App {
            volume,
            new_dir_name,
            pending_delete,
            ..
        } = &mut *app;
        let v = volume.as_ref().expect("volume present");
        let writable = v.writable;

        // Location row.
        ui.horizontal(|ui| {
            if ui
                .add_enabled(v.cwd != "/", egui::Button::new(l.up))
                .clicked()
            {
                action = Some(Action::Up);
            }
            theme::icon(ui, ic::FILES, 13.0, theme::ACCENT);
            ui.monospace(&v.cwd);
        });

        ui.add_space(theme::space::SM);

        // Action row.
        ui.horizontal(|ui| {
            if ui.add_enabled(writable, egui::Button::new(l.add)).clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    action = Some(Action::AddFile(path));
                }
            }
            ui.add_enabled_ui(writable, |ui| {
                ui.add(
                    egui::TextEdit::singleline(new_dir_name)
                        .desired_width(150.0)
                        .hint_text(l.new_folder_hint),
                );
                if ui.button(l.new_folder).clicked() {
                    action = Some(Action::Mkdir);
                }
            });
        });

        if !writable {
            ui.add_space(theme::space::TIGHT);
            ui.weak(l.read_only);
        }

        ui.add_space(theme::space::SM);
        ui.separator();
        ui.add_space(theme::space::SM);

        if let Some(err) = &v.list_error {
            ui.colored_label(theme::danger_ink(ui), l.cannot_list.replace("{err}", err));
        } else {
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    if v.entries.is_empty() {
                        ui.weak(l.empty);
                    }
                    for e in &v.entries {
                        let full = vfsops::join(&v.cwd, &e.name);
                        ui.horizontal(|ui| {
                            if e.is_dir {
                                theme::icon(ui, ic::ARROW_RIGHT, 12.0, theme::ACCENT);
                                if ui.button(format!("{}/", e.name)).clicked() {
                                    action = Some(Action::Navigate(full.clone()));
                                }
                            } else {
                                ui.label(&e.name);
                                ui.weak(vfsops::human_size(e.size));
                                if ui.button(l.extract).clicked() {
                                    if let Some(into) = rfd::FileDialog::new().pick_folder() {
                                        action = Some(Action::Extract {
                                            src: full.clone(),
                                            into,
                                        });
                                    }
                                }
                                if writable {
                                    if pending_delete.as_deref() == Some(full.as_str()) {
                                        if ui.button(l.delete).clicked() {
                                            action = Some(Action::DoDelete(full.clone()));
                                        }
                                        if ui.button(l.cancel).clicked() {
                                            action = Some(Action::CancelDelete);
                                        }
                                    } else if ui.button(l.delete).clicked() {
                                        action = Some(Action::AskDelete(full.clone()));
                                    }
                                }
                            }
                        });
                    }
                });
        }

        if !v.op_status.is_empty() {
            ui.add_space(theme::space::SM);
            ui.weak(&v.op_status);
        }
    });

    if let Some(action) = action {
        perform(app, action);
    }
}

fn perform(app: &mut App, action: Action) {
    match action {
        Action::Navigate(dir) => {
            if let Some(v) = app.volume.as_mut() {
                v.cwd = dir;
            }
            app.pending_delete = None;
            app.refresh_listing();
        }
        Action::Up => {
            if let Some(v) = app.volume.as_mut() {
                v.cwd = vfsops::parent_of(&v.cwd);
            }
            app.pending_delete = None;
            app.refresh_listing();
        }
        Action::AddFile(local) => {
            let dir = current_dir(app);
            let tmpl = app.tr("Added {name} ({size})");
            with_vfs(app, |vfs| {
                let n = vfsops::add_file(vfs, &local, &dir)?;
                let name = local
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Ok(tmpl
                    .replace("{name}", &name)
                    .replace("{size}", &vfsops::human_size(n)))
            });
            app.refresh_listing();
        }
        Action::Mkdir => {
            let parent = current_dir(app);
            let name = app.new_dir_name.trim().to_owned();
            if name.is_empty() {
                let msg = app.tr("Enter a folder name first.").to_owned();
                set_status(app, msg);
                return;
            }
            let tmpl = app.tr("Created {name}/");
            with_vfs(app, |vfs| {
                vfsops::mkdir(vfs, &parent, &name)?;
                Ok(tmpl.replace("{name}", &name))
            });
            app.new_dir_name.clear();
            app.refresh_listing();
        }
        Action::Extract { src, into } => {
            let tmpl = app.tr("Extracted {n} file(s) to {path}");
            with_vfs(app, |vfs| {
                let files = vfsops::extract(vfs, &src, &into)?;
                Ok(tmpl
                    .replace("{n}", &files.to_string())
                    .replace("{path}", &into.display().to_string()))
            });
        }
        Action::AskDelete(path) => {
            app.pending_delete = Some(path);
        }
        Action::CancelDelete => {
            app.pending_delete = None;
        }
        Action::DoDelete(path) => {
            let tmpl = app.tr("Deleted {name}");
            with_vfs(app, |vfs| {
                vfsops::delete_file(vfs, &path)?;
                Ok(tmpl.replace("{name}", vfsops::basename(&path)))
            });
            app.pending_delete = None;
            app.refresh_listing();
        }
    }
}

fn current_dir(app: &App) -> String {
    app.volume
        .as_ref()
        .map(|v| v.cwd.clone())
        .unwrap_or_else(|| "/".to_owned())
}

/// Run a bridge operation against the open volume and record its outcome.
fn with_vfs(app: &mut App, f: impl FnOnce(&mut dyn vc_fs::Vfs) -> anyhow::Result<String>) {
    let err_tmpl = app.tr("Error: {e}");
    if let Some(v) = app.volume.as_mut() {
        match f(v.session.vfs()) {
            Ok(msg) => v.op_status = msg,
            Err(e) => v.op_status = err_tmpl.replace("{e}", &format!("{e:#}")),
        }
    }
}

fn set_status(app: &mut App, msg: String) {
    if let Some(v) = app.volume.as_mut() {
        v.op_status = msg;
    }
}
