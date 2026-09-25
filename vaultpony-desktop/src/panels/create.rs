//! Create panel: the new-container wizard.
//!
//! Choose a location, size, cipher, hash, filesystem, PIM, passphrase, and
//! optional keyfiles, then create a VeraCrypt-compatible container. The work
//! (header-key derivation and formatting) runs on a worker thread, so the
//! window stays responsive; on success the new container is prefilled on the
//! Volumes tab, ready to unlock. Hidden volumes arrive in P3.

use eframe::egui::{self, Ui};
use vault_core::ContainerFs;
use zeroize::Zeroize;
use zeroize::Zeroizing;

use crate::app::{App, SizeUnit};
use crate::i18n::Key;
use crate::theme;
use crate::{newvolume, tasks};

/// The smallest container the wizard offers. The core needs at least a header
/// group plus a usable data area; 1 MiB clears that with room to spare.
const MIN_SIZE: u64 = 1024 * 1024;

pub fn ui(app: &mut App, ui: &mut Ui) {
    theme::screen_head(ui, app.t(Key::TabCreate), app.t(Key::SubCreate), |_ui| {});

    let busy = app.create_job.is_some();

    egui::ScrollArea::vertical().show(ui, |ui| {
        // Location.
        ui.horizontal(|ui| {
            if ui.button(app.tr("Choose location...")).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("vault.hc")
                    .save_file()
                {
                    app.create.out_path = Some(path);
                }
            }
            let shown = app
                .create
                .out_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| app.tr("no location chosen").to_owned());
            ui.label(shown);
        });

        ui.add_space(6.0);

        // Size.
        ui.horizontal(|ui| {
            ui.label(app.t(Key::Size));
            ui.add(
                egui::TextEdit::singleline(&mut app.create.size_value)
                    .desired_width(80.0)
                    .hint_text("10"),
            );
            egui::ComboBox::from_id_salt("size_unit")
                .selected_text(app.create.size_unit.label())
                .show_ui(ui, |ui| {
                    for unit in SizeUnit::ALL {
                        ui.selectable_value(&mut app.create.size_unit, unit, unit.label());
                    }
                });
        });

        ui.add_space(6.0);

        // Cipher and hash.
        ui.horizontal(|ui| {
            egui::ComboBox::from_label(app.t(Key::Cipher))
                .selected_text(&app.create.scheme)
                .show_ui(ui, |ui| {
                    for name in newvolume::scheme_names() {
                        ui.selectable_value(&mut app.create.scheme, name.to_owned(), name);
                    }
                });
        });
        ui.horizontal(|ui| {
            egui::ComboBox::from_label(app.t(Key::Hash))
                .selected_text(&app.create.prf)
                .show_ui(ui, |ui| {
                    for name in newvolume::prf_names() {
                        ui.selectable_value(&mut app.create.prf, name.to_owned(), name);
                    }
                });
        });
        if app.create.prf == "Argon2id" {
            ui.small(app.tr("Argon2id is memory-hard: at the default PIM, opening this vault needs about 416 MB of free memory and takes a few seconds. VeraCrypt opens it from version 1.26.29."));
        }

        ui.add_space(6.0);

        // Filesystem.
        ui.horizontal(|ui| {
            ui.label(app.t(Key::Filesystem));
            ui.radio_value(&mut app.create.fs, ContainerFs::Fat, "FAT");
            ui.radio_value(&mut app.create.fs, ContainerFs::Exfat, "exFAT");
        });
        ui.small(app.tr("FAT is universal; exFAT lifts the 4 GiB per-file limit (read-only in-app until the core's exFAT write lands)."));

        ui.add_space(6.0);

        // PIM.
        ui.horizontal(|ui| {
            ui.label(app.t(Key::Pim));
            ui.add(
                egui::TextEdit::singleline(&mut app.create.pim)
                    .desired_width(80.0)
                    .hint_text("0"),
            );
            ui.small(app.tr("blank or 0 uses the default iteration schedule"));
        });

        ui.add_space(6.0);

        // Passphrase and confirmation.
        ui.horizontal(|ui| {
            ui.label(app.t(Key::Passphrase));
            let (show, hide) = (app.tr("Show"), app.tr("Hide"));
            theme::password_edit(ui, "create", &mut app.create.pass, 240.0, show, hide);
        });
        ui.horizontal(|ui| {
            ui.label(app.t(Key::Confirm));
            let (show, hide) = (app.tr("Show"), app.tr("Hide"));
            theme::password_edit(ui, "create-confirm", &mut app.create.pass2, 240.0, show, hide);
        });

        ui.add_space(6.0);

        // Keyfiles.
        ui.horizontal(|ui| {
            if ui.button(app.tr("Add keyfile...")).clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    app.create.keyfiles.push(path);
                }
            }
            if !app.create.keyfiles.is_empty() && ui.button(app.tr("Clear keyfiles")).clicked() {
                app.create.keyfiles.clear();
            }
            ui.label(
                app.tr("{n} keyfile(s)")
                    .replace("{n}", &app.create.keyfiles.len().to_string()),
            );
        });
        for kf in &app.create.keyfiles {
            ui.small(kf.display().to_string());
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(8.0);

        // Hidden volume.
        let hidden_label = app.t(Key::HiddenVolume);
        ui.checkbox(&mut app.create.hidden, hidden_label);
        if app.create.hidden {
            ui.small(app.tr("Reuses the cipher, hash, and filesystem above. It gets its own passphrase, PIM, and size, and must use a different passphrase from the outer volume."));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(app.tr("Hidden size"));
                ui.add(
                    egui::TextEdit::singleline(&mut app.create.hidden_size_value)
                        .desired_width(80.0)
                        .hint_text("2"),
                );
                egui::ComboBox::from_id_salt("hidden_size_unit")
                    .selected_text(app.create.hidden_size_unit.label())
                    .show_ui(ui, |ui| {
                        for unit in SizeUnit::ALL {
                            ui.selectable_value(
                                &mut app.create.hidden_size_unit,
                                unit,
                                unit.label(),
                            );
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label(app.t(Key::HiddenPassphrase));
                let (show, hide) = (app.tr("Show"), app.tr("Hide"));
                theme::password_edit(ui, "create-hidden", &mut app.create.hidden_pass, 240.0, show, hide);
            });
            ui.horizontal(|ui| {
                ui.label(app.t(Key::Confirm));
                let (show, hide) = (app.tr("Show"), app.tr("Hide"));
                theme::password_edit(ui, "create-hidden-confirm", &mut app.create.hidden_pass2, 240.0, show, hide);
            });
            ui.horizontal(|ui| {
                ui.label(app.tr("Hidden PIM"));
                ui.add(
                    egui::TextEdit::singleline(&mut app.create.hidden_pim)
                        .desired_width(80.0)
                        .hint_text("0"),
                );
            });
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);
        }

        ui.horizontal(|ui| {
            if theme::primary_button_enabled(ui, app.t(Key::CreateContainer), !busy).clicked() {
                let ctx = ui.ctx().clone();
                start_create(app, &ctx);
            }
            if busy {
                ui.spinner();
                ui.label(app.tr("Creating..."));
            }
        });

        if !app.create.status.is_empty() {
            ui.add_space(6.0);
            ui.label(&app.create.status);
        }
    });
}

fn start_create(app: &mut App, ctx: &egui::Context) {
    let Some(path) = app.create.out_path.clone() else {
        app.create.status = app
            .tr("Choose where to save the container first.")
            .to_owned();
        return;
    };
    let Some(size) = app.create.size_unit.to_bytes(&app.create.size_value) else {
        app.create.status = app.tr("Enter a valid size.").to_owned();
        return;
    };
    if size < MIN_SIZE {
        app.create.status = app.tr("Container must be at least 1 MiB.").to_owned();
        return;
    }
    if app.create.pass != app.create.pass2 {
        app.create.status = app.tr("Passphrases do not match.").to_owned();
        return;
    }
    if app.create.pass.is_empty() && app.create.keyfiles.is_empty() {
        app.create.status = app
            .tr("Set a passphrase or add at least one keyfile.")
            .to_owned();
        return;
    }
    let Some(scheme) = newvolume::scheme_by_name(&app.create.scheme) else {
        app.create.status = app
            .tr("Unknown cipher: {name}")
            .replace("{name}", &app.create.scheme);
        return;
    };
    let Some(prf) = newvolume::prf_by_name(&app.create.prf) else {
        app.create.status = app
            .tr("Unknown hash: {name}")
            .replace("{name}", &app.create.prf);
        return;
    };
    let pim = app.create.pim.trim().parse::<u32>().unwrap_or(0);

    // Hidden volume, if requested.
    let hidden = if app.create.hidden {
        let Some(hidden_size) = app
            .create
            .hidden_size_unit
            .to_bytes(&app.create.hidden_size_value)
        else {
            app.create.status = app.tr("Enter a valid hidden-volume size.").to_owned();
            return;
        };
        if hidden_size >= size {
            app.create.status = app
                .tr("The hidden volume must be smaller than the container.")
                .to_owned();
            return;
        }
        if app.create.hidden_pass != app.create.hidden_pass2 {
            app.create.status = app.tr("Hidden passphrases do not match.").to_owned();
            return;
        }
        if app.create.hidden_pass.is_empty() {
            app.create.status = app.tr("Set a passphrase for the hidden volume.").to_owned();
            return;
        }
        if app.create.hidden_pass == app.create.pass {
            app.create.status = app
                .tr("The hidden volume must use a different passphrase from the outer volume.")
                .to_owned();
            return;
        }
        let hidden_pim = app.create.hidden_pim.trim().parse::<u32>().unwrap_or(0);
        Some(newvolume::HiddenPart {
            passphrase: Zeroizing::new(app.create.hidden_pass.clone().into_bytes()),
            pim: hidden_pim,
            keyfiles: Vec::new(),
            data_size: hidden_size,
        })
    } else {
        None
    };

    let spec = newvolume::Spec {
        path,
        size,
        scheme,
        prf,
        pim,
        fs: app.create.fs,
        passphrase: Zeroizing::new(app.create.pass.clone().into_bytes()),
        keyfiles: app.create.keyfiles.clone(),
        hidden,
    };

    app.create.pass.zeroize();
    app.create.pass2.zeroize();
    app.create.hidden_pass.zeroize();
    app.create.hidden_pass2.zeroize();
    app.create.status = app.tr("Creating...").to_owned();
    app.create_job = Some(tasks::spawn_create(ctx.clone(), spec));
}
