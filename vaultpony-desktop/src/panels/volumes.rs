//! Volumes panel: unlock a container, see what opened it, lock or mount it.

use eframe::egui::{self, Ui};

use crate::app::App;
use crate::i18n::Key;
use crate::theme::{self, ic};

pub fn ui(app: &mut App, ui: &mut Ui) {
    theme::screen_head(ui, app.t(Key::TabVolumes), app.t(Key::SubVolumes), |_ui| {});

    if app.mount.is_some() {
        theme::card(ui, |ui| mounted_view(app, ui));
        ui.add_space(theme::space::MD);
    }

    theme::card(ui, |ui| unlock_form(app, ui));

    if app.mount.is_none() {
        ui.add_space(theme::space::MD);
        if app.volume.is_some() {
            theme::card(ui, |ui| open_volume(app, ui));
        } else {
            theme::empty_state(ui, ic::LOCK, app.t(Key::NoVolumeOpen));
        }
    }
}

fn mounted_view(app: &mut App, ui: &mut Ui) {
    theme::section(ui, app.t(crate::i18n::Key::SecMounted));
    ui.add_space(theme::space::SM);
    let mountpoint = app
        .mount
        .as_ref()
        .map(|m| m.mountpoint().display().to_string())
        .unwrap_or_default();
    ui.horizontal(|ui| {
        theme::icon(ui, ic::CHECK, 14.0, theme::ACCENT);
        ui.monospace(mountpoint);
    });
    ui.add_space(theme::space::TIGHT);
    ui.weak(app.tr("Open the mount point in Finder to use the volume. It is mounted read-only."));
    ui.add_space(theme::space::MD);
    if theme::secondary_button(ui, app.t(Key::Unmount)).clicked() {
        app.unmount();
    }
}

fn unlock_form(app: &mut App, ui: &mut Ui) {
    theme::section(ui, app.t(Key::Unlock));
    ui.add_space(theme::space::SM);
    let running = app.job.is_some();

    ui.horizontal(|ui| {
        if theme::secondary_button(ui, app.t(Key::ChooseContainer)).clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                app.container_path = Some(path);
            }
        }
        let shown = app
            .container_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| app.t(Key::NoFileChosen).to_owned());
        ui.weak(shown);
    });

    ui.add_space(theme::space::SM);
    ui.horizontal(|ui| {
        ui.label(app.t(Key::Passphrase));
        ui.add(
            egui::TextEdit::singleline(&mut app.password)
                .password(true)
                .desired_width(240.0),
        );
    });

    ui.horizontal(|ui| {
        ui.label(app.t(Key::Pim));
        ui.add(
            egui::TextEdit::singleline(&mut app.pim)
                .desired_width(80.0)
                .hint_text("0"),
        );
        ui.weak(app.t(Key::HintPimDefault));
    });

    ui.add_space(theme::space::SM);
    let protect_label = app.t(Key::ProtectHiddenCheck);
    ui.checkbox(&mut app.protect_hidden, protect_label);
    if app.protect_hidden {
        ui.horizontal(|ui| {
            ui.label(app.t(Key::HiddenPassphrase));
            ui.add(
                egui::TextEdit::singleline(&mut app.hidden_password)
                    .password(true)
                    .desired_width(240.0),
            );
        });
        ui.weak(app.tr("A write that would hit the hidden region is refused."));
    } else {
        ui.weak(app.t(Key::HintHiddenSelect));
    }

    ui.add_space(theme::space::MD);
    ui.horizontal(|ui| {
        let can_unlock = !running && app.container_path.is_some();
        if theme::primary_button_enabled(ui, app.t(Key::Unlock), can_unlock).clicked() {
            let ctx = ui.ctx().clone();
            app.start_unlock(&ctx);
        }
        if running {
            ui.spinner();
        }
    });

    if !app.status.is_empty() {
        ui.add_space(theme::space::SM);
        let err = is_error(&app.status);
        theme::status_line(ui, &app.status, err);
    }
}

fn open_volume(app: &mut App, ui: &mut Ui) {
    let mut lock_clicked = false;
    let mut mount_clicked = false;

    if let Some(v) = &app.volume {
        let mode = if v.writable {
            app.t(Key::ReadWrite)
        } else {
            app.t(Key::ReadOnly)
        };
        let name = v
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "container".to_owned());

        theme::section(ui, app.t(Key::SecOpenVolume));
        ui.add_space(theme::space::SM);
        ui.horizontal(|ui| {
            theme::icon(ui, ic::LOCK_OPEN, 15.0, theme::ACCENT);
            ui.label(egui::RichText::new(name).font(theme::semibold(14.0)));
        });
        ui.add_space(theme::space::TIGHT);
        ui.weak(format!(
            "{} / {}   ·   {:?}   ·   {}",
            v.scheme, v.prf, v.kind, mode
        ));

        if v.protected {
            ui.add_space(theme::space::TIGHT);
            ui.colored_label(
                theme::ACCENT,
                app.tr(
                    "Hidden-volume protection is on: writes into the hidden region are refused.",
                ),
            );
        }
        ui.add_space(theme::space::SM);
        ui.weak(app.t(Key::HintOpenFilesTab));

        ui.add_space(theme::space::MD);
        ui.horizontal(|ui| {
            if theme::secondary_button(ui, app.t(Key::Lock)).clicked() {
                lock_clicked = true;
            }
            match crate::mount::unavailable_reason() {
                None => {
                    if theme::primary_button(ui, app.t(Key::MountAsDrive)).clicked() {
                        mount_clicked = true;
                    }
                }
                Some(_) => {
                    theme::primary_button_enabled(ui, app.t(Key::MountAsDrive), false);
                }
            }
        });
        if let Some(reason) = crate::mount::unavailable_reason() {
            ui.add_space(theme::space::TIGHT);
            ui.weak(reason);
        }
    }

    if lock_clicked {
        app.lock();
    } else if mount_clicked {
        app.start_mount();
    }
}

fn is_error(status: &str) -> bool {
    let s = status.to_lowercase();
    s.contains("could not") || s.contains("fail") || s.contains("error") || s.contains("wrong")
}
