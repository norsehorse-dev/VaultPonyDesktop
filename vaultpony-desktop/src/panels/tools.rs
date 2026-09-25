//! Tools panel: volume info, header backup and restore, and change password.
//!
//! Each action works on the container file directly (no unlocked session
//! needed) and runs on a worker thread through `crate::tools`. The volume-info
//! output is support-safe: parameters only, never names, paths, or key
//! material.

use eframe::egui::{self, Ui};
use zeroize::{Zeroize, Zeroizing};

use crate::app::App;
use crate::i18n::Key;
use crate::theme;
use crate::{tasks, tools};

pub fn ui(app: &mut App, ui: &mut Ui) {
    theme::screen_head(ui, app.t(Key::TabTools), app.t(Key::SubTools), |_ui| {});

    let busy = app.tool_job.is_some();
    let ctx = ui.ctx().clone();

    // Target container.
    ui.horizontal(|ui| {
        if ui.button(app.tr("Choose container...")).clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                app.container_path = Some(path);
            }
        }
        let shown = app
            .container_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| app.tr("no container chosen").to_owned());
        ui.label(shown);
    });

    let Some(container) = app.container_path.clone() else {
        ui.add_space(8.0);
        ui.label(app.tr("Choose a container to use the header tools."));
        return;
    };

    if app.volume.is_some() {
        ui.add_space(4.0);
        ui.colored_label(
            egui::Color32::from_rgb(0xE0, 0xB0, 0x50),
            app.tr("A volume is open. Lock it before restoring or changing headers."),
        );
    }

    // Current passphrase and PIM: used by info, restore, and the old side of a
    // password change.
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(app.t(Key::Passphrase));
        let (show, hide) = (app.tr("Show"), app.tr("Hide"));
        theme::password_edit(ui, "tools-current", &mut app.tools.pass, 240.0, show, hide);
    });
    ui.horizontal(|ui| {
        ui.label(app.t(Key::Pim));
        ui.add(
            egui::TextEdit::singleline(&mut app.tools.pim)
                .desired_width(80.0)
                .hint_text("0"),
        );
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(!busy, egui::Button::new(app.t(Key::ShowVolumeInfo)))
            .clicked()
        {
            let secret = Zeroizing::new(app.tools.pass.clone().into_bytes());
            let pim = parse_pim(&app.tools.pim);
            let c = container.clone();
            app.tools.output = app.tr("Working...").to_owned();
            app.tool_job = Some(tasks::spawn_tool(ctx.clone(), move || {
                tools::info(&c, &secret, pim).map_err(|e| format!("{e:#}"))
            }));
        }
        if ui
            .add_enabled(!busy, egui::Button::new(app.t(Key::BackUpHeader)))
            .clicked()
        {
            if let Some(out) = rfd::FileDialog::new()
                .set_file_name("header.hcbak")
                .save_file()
            {
                let c = container.clone();
                let ok_msg = app
                    .tr("Backed up header to {path}")
                    .replace("{path}", &out.display().to_string());
                app.tools.output = app.tr("Working...").to_owned();
                app.tool_job = Some(tasks::spawn_tool(ctx.clone(), move || {
                    tools::backup_header(&c, &out)
                        .map(|()| ok_msg)
                        .map_err(|e| format!("{e:#}"))
                }));
            }
        }
    });
    ui.small(app.tr("A header backup keeps accepting the current password even after a password change. Store it as carefully as the container itself."));

    // Restore.
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
    ui.strong(app.t(Key::SecRestoreHeader));
    ui.horizontal(|ui| {
        if ui
            .add_enabled(!busy, egui::Button::new(app.t(Key::RestoreFromFile)))
            .clicked()
        {
            if let Some(backup) = rfd::FileDialog::new().pick_file() {
                let secret = Zeroizing::new(app.tools.pass.clone().into_bytes());
                let pim = parse_pim(&app.tools.pim);
                let c = container.clone();
                let ok_msg = app
                    .tr("Restored the primary header from the backup file.")
                    .to_owned();
                app.tools.output = app.tr("Working...").to_owned();
                app.tool_job = Some(tasks::spawn_tool(ctx.clone(), move || {
                    tools::restore_from_file(&c, &backup, &secret, pim)
                        .map(|()| ok_msg)
                        .map_err(|e| format!("{e:#}"))
                }));
            }
        }
        if ui
            .add_enabled(!busy, egui::Button::new(app.t(Key::RestoreFromEmbedded)))
            .clicked()
        {
            let secret = Zeroizing::new(app.tools.pass.clone().into_bytes());
            let pim = parse_pim(&app.tools.pim);
            let c = container.clone();
            let ok_msg = app
                .tr("Restored the primary header from the embedded backup.")
                .to_owned();
            app.tools.output = app.tr("Working...").to_owned();
            app.tool_job = Some(tasks::spawn_tool(ctx.clone(), move || {
                tools::restore_from_embedded(&c, &secret, pim)
                    .map(|()| ok_msg)
                    .map_err(|e| format!("{e:#}"))
            }));
        }
    });
    ui.small(app.tr("Restore verifies the replacement header unlocks with the passphrase above before writing anything."));

    // Change password.
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
    ui.strong(app.t(Key::ChangePassword));
    ui.small(app.tr("The passphrase and PIM above are the current (old) secret."));
    ui.horizontal(|ui| {
        ui.label(app.tr("New passphrase"));
        let (show, hide) = (app.tr("Show"), app.tr("Hide"));
        theme::password_edit(ui, "tools-new", &mut app.tools.new_pass, 240.0, show, hide);
    });
    ui.horizontal(|ui| {
        ui.label(app.t(Key::Confirm));
        let (show, hide) = (app.tr("Show"), app.tr("Hide"));
        theme::password_edit(
            ui,
            "tools-confirm",
            &mut app.tools.new_pass2,
            240.0,
            show,
            hide,
        );
    });
    ui.horizontal(|ui| {
        ui.label(app.tr("New PIM"));
        ui.add(
            egui::TextEdit::singleline(&mut app.tools.new_pim)
                .desired_width(80.0)
                .hint_text("0"),
        );
    });
    // Optionally move the header to another KDF (Argon2id, VeraCrypt 1.26.29+).
    // Switching clears the new PIM so the new KDF's own default applies.
    ui.horizontal(|ui| {
        ui.label(app.tr("Key derivation"));
        let keep = app.tr("Keep current");
        let before = app.tools.new_kdf.clone();
        egui::ComboBox::from_id_salt("tools-new-kdf")
            .selected_text(app.tools.new_kdf.as_deref().unwrap_or(keep))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut app.tools.new_kdf, None, keep);
                for name in crate::newvolume::prf_names() {
                    ui.selectable_value(&mut app.tools.new_kdf, Some(name.to_owned()), name);
                }
            });
        if app.tools.new_kdf != before {
            app.tools.new_pim.clear();
        }
    });
    if app.tools.new_kdf.is_some() {
        ui.small(app.tr("Only the header is re-encrypted with the new KDF. Leave the new PIM empty to use its default."));
    }
    if ui
        .add_enabled(!busy, egui::Button::new(app.t(Key::ChangePassword)))
        .clicked()
    {
        if app.tools.new_pass != app.tools.new_pass2 {
            app.tools.output = app.tr("New passphrases do not match.").to_owned();
        } else if app.tools.new_pass.is_empty() {
            app.tools.output = app.tr("Enter a new passphrase.").to_owned();
        } else {
            let old = Zeroizing::new(app.tools.pass.clone().into_bytes());
            let old_pim = parse_pim(&app.tools.pim);
            let new = Zeroizing::new(app.tools.new_pass.clone().into_bytes());
            let new_pim = parse_pim(&app.tools.new_pim);
            let new_kdf = app.tools.new_kdf.clone();
            let c = container.clone();
            let ok_msg = app
                .tr("Password changed. Use the new passphrase from now on.")
                .to_owned();
            app.tools.new_pass.zeroize();
            app.tools.new_pass2.zeroize();
            app.tools.output = app.tr("Working...").to_owned();
            app.tool_job = Some(tasks::spawn_tool(ctx.clone(), move || {
                tools::change_password(&c, &old, old_pim, &new, new_pim, new_kdf.as_deref())
                    .map(|()| ok_msg)
                    .map_err(|e| format!("{e:#}"))
            }));
        }
    }

    if busy {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(app.tr("Working..."));
        });
    }

    if !app.tools.output.is_empty() {
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                ui.monospace(&app.tools.output);
            });
    }
}

fn parse_pim(s: &str) -> u32 {
    s.trim().parse::<u32>().unwrap_or(0)
}
