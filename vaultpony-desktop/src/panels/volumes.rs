//! Volumes panel.
//!
//! For P0 this is the whole app: pick a container, unlock it, and browse the
//! decrypted directory tree. The mount table (many volumes open at once, each
//! optionally mounted as a real drive) replaces the single-volume view in P5.

use eframe::egui::{self, Ui};

use crate::app::{App, Volume};

pub fn ui(app: &mut App, ui: &mut Ui) {
    ui.add_space(8.0);
    ui.heading("Volumes");
    ui.add_space(8.0);

    unlock_form(app, ui);

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    if app.volume.is_some() {
        open_volume(app, ui);
    } else {
        ui.label("No volume open.");
    }
}

fn unlock_form(app: &mut App, ui: &mut Ui) {
    let running = app.job.is_some();

    ui.horizontal(|ui| {
        if ui.button("Choose container...").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_file() {
                app.container_path = Some(path);
            }
        }
        let shown = app
            .container_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "no file chosen".to_owned());
        ui.label(shown);
    });

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Passphrase");
        ui.add(
            egui::TextEdit::singleline(&mut app.password)
                .password(true)
                .desired_width(240.0)
                .hint_text("Passphrase"),
        );
    });

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("PIM");
        ui.add(
            egui::TextEdit::singleline(&mut app.pim)
                .desired_width(80.0)
                .hint_text("0"),
        );
        ui.small("blank or 0 uses the default iteration schedule");
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        let can_unlock = !running && app.container_path.is_some();
        if ui
            .add_enabled(can_unlock, egui::Button::new("Unlock"))
            .clicked()
        {
            let ctx = ui.ctx().clone();
            app.start_unlock(&ctx);
        }
        if running {
            ui.spinner();
        }
    });

    if !app.status.is_empty() {
        ui.add_space(6.0);
        ui.label(&app.status);
    }
}

fn open_volume(app: &mut App, ui: &mut Ui) {
    // Pull display facts and decide navigation without holding a borrow across
    // the mutating calls (lock, refresh) below.
    let mut lock_clicked = false;
    let mut go: Option<String> = None;

    if let Some(v) = &app.volume {
        ui.horizontal(|ui| {
            ui.strong(
                v.path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "container".to_owned()),
            );
            if ui.button("Lock").clicked() {
                lock_clicked = true;
            }
        });
        ui.add_space(4.0);
        ui.label(format!(
            "{} / {}   filesystem {:?}   {}",
            v.scheme,
            v.prf,
            v.kind,
            if v.writable {
                "read-write"
            } else {
                "read-only"
            }
        ));

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(v.cwd != "/", egui::Button::new("Up"))
                .clicked()
            {
                go = Some(parent_of(&v.cwd));
            }
            ui.monospace(&v.cwd);
        });
        ui.add_space(6.0);

        if let Some(err) = &v.list_error {
            ui.colored_label(egui::Color32::LIGHT_RED, format!("Cannot list: {err}"));
        } else {
            go = go.or_else(|| listing_table(ui, v));
        }
    }

    if lock_clicked {
        app.lock();
        return;
    }
    if let Some(dir) = go {
        if let Some(v) = app.volume.as_mut() {
            v.cwd = dir;
        }
        app.refresh_listing();
    }
}

/// Render the directory listing. Returns the path to navigate into if the user
/// clicked a directory.
fn listing_table(ui: &mut Ui, v: &Volume) -> Option<String> {
    let mut go = None;
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("entries")
            .num_columns(2)
            .striped(true)
            .spacing([24.0, 4.0])
            .show(ui, |ui| {
                if v.entries.is_empty() {
                    ui.label("(empty)");
                    ui.end_row();
                }
                for e in &v.entries {
                    if e.is_dir {
                        if ui.button(format!("{}/", e.name)).clicked() {
                            go = Some(join(&v.cwd, &e.name));
                        }
                        ui.label("<dir>");
                    } else {
                        ui.label(&e.name);
                        ui.monospace(human_size(e.size));
                    }
                    ui.end_row();
                }
            });
    });
    go
}

fn join(cwd: &str, name: &str) -> String {
    if cwd == "/" {
        format!("/{name}")
    } else {
        format!("{cwd}/{name}")
    }
}

fn parent_of(cwd: &str) -> String {
    match cwd.rfind('/') {
        Some(0) | None => "/".to_owned(),
        Some(i) => cwd[..i].to_owned(),
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::{human_size, join, parent_of};

    #[test]
    fn join_from_root_and_deeper() {
        assert_eq!(join("/", "docs"), "/docs");
        assert_eq!(join("/docs", "taxes"), "/docs/taxes");
    }

    #[test]
    fn parent_walks_up_to_root() {
        assert_eq!(parent_of("/docs/taxes"), "/docs");
        assert_eq!(parent_of("/docs"), "/");
        assert_eq!(parent_of("/"), "/");
    }

    #[test]
    fn sizes_are_human() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1024), "1.0 KiB");
        assert_eq!(human_size(1536), "1.5 KiB");
        assert_eq!(human_size(1024 * 1024), "1.0 MiB");
    }
}
