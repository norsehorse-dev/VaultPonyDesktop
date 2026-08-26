//! Settings screen: appearance, language, privacy, Create defaults, and the
//! informational sections (About and source links, the pony family, help,
//! security, and licenses). Every link opens the browser; the app itself makes
//! no network request.

use eframe::egui::{self, FontId, RichText, Sense, Ui, Vec2};

use crate::app::{App, ThemeChoice};
use crate::i18n::{self, Key};
use crate::newvolume;
use crate::theme;

pub fn ui(app: &mut App, ui: &mut Ui) {
    theme::screen_head(
        ui,
        app.t(Key::TabSettings),
        app.t(Key::SubSettings),
        |_ui| {},
    );

    let before = app.settings.clone();

    egui::ScrollArea::vertical().show(ui, |ui| {
        appearance_and_language(app, ui);
        ui.add_space(theme::space::MD);
        privacy_and_lock(app, ui);
        ui.add_space(theme::space::MD);
        create_defaults(app, ui);
        ui.add_space(theme::space::MD);
        about(app, ui);
        ui.add_space(theme::space::MD);
        help(app, ui);
        ui.add_space(theme::space::MD);
        security(app, ui);
        ui.add_space(theme::space::MD);
        licenses(app, ui);
        ui.add_space(theme::space::MD);
        family(app, ui);
    });

    if app.settings != before {
        let _ = app.settings.save();
    }
}

fn appearance_and_language(app: &mut App, ui: &mut Ui) {
    theme::card(ui, |ui| {
        theme::section(ui, app.t(Key::SecAppearance));
        ui.add_space(theme::space::SM);
        let cur = ThemeChoice::from_code(&app.settings.theme);
        let sel = ThemeChoice::ALL.iter().position(|c| *c == cur).unwrap_or(2);
        let labels: Vec<&str> = ThemeChoice::ALL.iter().map(|c| app.tr(c.label())).collect();
        if let Some(i) = theme::segmented(ui, &labels, sel) {
            let choice = ThemeChoice::ALL[i];
            app.settings.theme = choice.code().to_owned();
            choice.apply(ui.ctx());
        }
        ui.add_space(theme::space::TIGHT);
        ui.weak(app.tr("Auto follows the system. The choice is remembered."));

        ui.add_space(theme::space::MD);
        theme::section(ui, app.t(Key::Language));
        ui.add_space(theme::space::SM);
        egui::ComboBox::from_id_salt("language")
            .selected_text(app.lang().native_name())
            .show_ui(ui, |ui| {
                for lang in i18n::Lang::ALL {
                    ui.selectable_value(
                        &mut app.settings.language,
                        lang.code().to_owned(),
                        lang.native_name(),
                    );
                }
            });
    });
}

fn privacy_and_lock(app: &mut App, ui: &mut Ui) {
    theme::card(ui, |ui| {
        theme::section(ui, app.t(Key::AutoLock));
        ui.add_space(theme::space::SM);
        let lock_after = app.tr("Lock after");
        let minutes = app.tr("minutes of inactivity");
        ui.horizontal(|ui| {
            ui.label(lock_after);
            ui.add(egui::DragValue::new(&mut app.settings.auto_lock_minutes).range(0..=240));
            ui.label(minutes);
        });
        ui.weak(app.tr("0 disables it. Locking unmounts any mounted drive and zeroizes the keys."));

        ui.add_space(theme::space::MD);
        theme::section(ui, app.t(Key::Privacy));
        ui.add_space(theme::space::SM);
        let hide_label = app.t(Key::HideOnUnfocus);
        let no_trace_label = app.t(Key::NoTraceCheck);
        ui.checkbox(&mut app.settings.obscure_on_unfocus, hide_label);
        ui.checkbox(&mut app.settings.no_trace, no_trace_label);
        ui.add_space(theme::space::TIGHT);
        ui.weak(app.tr("OS-native unlock (Touch ID / Windows Hello) is planned; it needs platform framework integration."));
    });
}

fn create_defaults(app: &mut App, ui: &mut Ui) {
    theme::card(ui, |ui| {
        theme::section(ui, app.t(Key::SecCreateDefaults));
        ui.add_space(theme::space::SM);
        egui::ComboBox::from_label(app.t(Key::Cipher))
            .selected_text(&app.settings.default_scheme)
            .show_ui(ui, |ui| {
                for name in newvolume::scheme_names() {
                    ui.selectable_value(&mut app.settings.default_scheme, name.to_owned(), name);
                }
            });
        egui::ComboBox::from_label(app.t(Key::Hash))
            .selected_text(&app.settings.default_prf)
            .show_ui(ui, |ui| {
                for name in newvolume::prf_names() {
                    ui.selectable_value(&mut app.settings.default_prf, name.to_owned(), name);
                }
            });
    });
}

fn about(app: &mut App, ui: &mut Ui) {
    theme::card(ui, |ui| {
        theme::section(ui, app.t(Key::SecAbout));
        ui.add_space(theme::space::SM);
        ui.label(
            RichText::new(format!("VaultPony Desktop {}", env!("VP_VERSION")))
                .font(theme::semibold(14.0))
                .color(theme::ink(ui)),
        );
        ui.add_space(theme::space::TIGHT);
        ui.weak(app.tr(
            "Open and edit VeraCrypt-compatible encrypted containers, entirely on this computer. No accounts, no telemetry, no network access.",
        ));
        ui.add_space(theme::space::SM);
        ui.horizontal(|ui| {
            ui.hyperlink_to("vaultpony.app", "https://vaultpony.app");
            ui.label("·");
            ui.hyperlink_to(
                app.tr("Source (VaultPonyDesktop)"),
                "https://github.com/norsehorse-dev/VaultPonyDesktop",
            );
        });
        ui.horizontal(|ui| {
            ui.hyperlink_to(
                app.tr("Source (VaultPonyCore)"),
                "https://github.com/norsehorse-dev/VaultPonyCore",
            );
        });
        ui.add_space(theme::space::TIGHT);
        ui.weak(app.tr("Apache-2.0. VeraCrypt is a trademark of IDRIX; VaultPony is not affiliated with IDRIX."));
    });
}

/// FAQ entries (question, answer). Text is routed through `tr` at render time.
const FAQ: &[(&str, &str)] = &[
    (
        "What is a container?",
        "A container is a single encrypted file. Open it with your passphrase and it \
         behaves like a small disk you can put files inside. VaultPony's containers are \
         VeraCrypt-compatible, so desktop VeraCrypt opens what VaultPony makes, and the \
         reverse.",
    ),
    (
        "What is a hidden volume?",
        "A second container concealed inside the free space of the first. It has its own \
         password, and the outer volume gives no sign the hidden one exists. Which volume \
         opens is decided by the password you type; there is no toggle to reveal it.",
    ),
    (
        "What is a keyfile, and what is PIM?",
        "A keyfile is any file folded into your passphrase, so opening the container needs \
         both. PIM is a number that tunes how long key derivation takes; leave it blank for \
         the default. Both must match what the container was created with.",
    ),
    (
        "Can VeraCrypt open these containers?",
        "Yes. VaultPony writes the VeraCrypt volume format, so desktop VeraCrypt and other \
         compatible tools open containers VaultPony creates, with the matching password.",
    ),
    (
        "What does mounting do?",
        "Mounting presents an open container to your system as a real drive your other apps \
         can browse. It needs macFUSE installed and a build with the mount feature; without \
         it, the Files tab browses the container inside the app.",
    ),
];

fn help(app: &App, ui: &mut Ui) {
    theme::card(ui, |ui| {
        theme::section(ui, app.t(Key::SecHelp));
        ui.add_space(theme::space::SM);
        for (q, a) in FAQ {
            egui::CollapsingHeader::new(app.tr(q)).show(ui, |ui| {
                ui.weak(app.tr(a));
            });
        }
    });
}

/// Security explainer blocks (heading, body), routed through `tr` at render.
const BLOCKS: &[(&str, &str)] = &[
    (
        "Everything stays on this computer",
        "VaultPony makes no network requests at all. Opening, editing, and your passwords \
         never leave the machine. The links on this screen open your browser; the app \
         itself never connects.",
    ),
    (
        "The VeraCrypt format",
        "Containers use the VeraCrypt on-disk format: your chosen cipher (including \
         cascades) with XTS, and PBKDF2 key derivation at the PRF and PIM you pick. This is \
         a clean-room implementation from the published format.",
    ),
    (
        "Secrets in memory",
        "Passphrases and keys are zeroized when a container locks, through one lock path. \
         Auto-lock and unmount run that same path.",
    ),
];

/// The "what it does not do" bullets, routed through `tr` at render.
const NOT_DOING: &[&str] = &[
    "It does not back your containers up anywhere. Keep your own copies.",
    "It cannot recover a container whose password you have lost.",
    "It sends no analytics or telemetry, ever.",
    "It cannot protect files after they leave a container, or defend a computer already compromised at the system level.",
];

fn security(app: &App, ui: &mut Ui) {
    theme::card(ui, |ui| {
        theme::section(ui, app.t(Key::SecSecurity));
        ui.add_space(theme::space::SM);
        for (title, body) in BLOCKS {
            ui.label(
                RichText::new(app.tr(title))
                    .font(theme::semibold(13.5))
                    .color(theme::ink(ui)),
            );
            ui.weak(app.tr(body));
            ui.add_space(theme::space::SM);
        }
        ui.label(
            RichText::new(app.tr("What VaultPony does not do"))
                .font(theme::semibold(13.5))
                .color(theme::ink(ui)),
        );
        ui.add_space(theme::space::TIGHT);
        for line in NOT_DOING {
            ui.weak(format!("\u{2022}  {}", app.tr(line)));
        }
    });
}

fn licenses(app: &App, ui: &mut Ui) {
    const CRATES: &[(&str, &str)] = &[
        ("VaultPonyCore (the container implementation)", "Apache-2.0"),
        ("egui, eframe (the user interface)", "MIT or Apache-2.0"),
        (
            "Inter, JetBrains Mono, Lucide (fonts and icons)",
            "OFL-1.1 / ISC",
        ),
        ("DejaVu (Cyrillic fallback)", "permissive (Bitstream Vera)"),
        (
            "serde, rfd, directories, anyhow, zeroize",
            "MIT or Apache-2.0",
        ),
        ("fuser (optional mount backend)", "MIT or Apache-2.0"),
    ];
    theme::card(ui, |ui| {
        theme::section(ui, app.t(Key::SecLicenses));
        ui.add_space(theme::space::SM);
        ui.weak(app.tr(
            "VaultPony Desktop is Apache-2.0 and builds on these, each under its own license. macFUSE, if you install it for mounting, is a separate third-party component and is never bundled.",
        ));
        ui.add_space(theme::space::SM);
        for (name, license) in CRATES {
            ui.label(
                RichText::new(app.tr(name))
                    .font(theme::semibold(12.5))
                    .color(theme::ink(ui)),
            );
            ui.weak(*license);
            ui.add_space(theme::space::TIGHT);
        }
    });
}

/// One sibling app: name, tagline, platforms, url, and its site's accent.
struct PonyApp {
    name: &'static str,
    tagline: &'static str,
    platforms: &'static str,
    url: &'static str,
    accent: egui::Color32,
}

/// The rest of the pony family (VaultPony excluded, AgePony included). Accents
/// lifted from the family band on the sites so the lists agree.
const FAMILY: &[PonyApp] = &[
    PonyApp {
        name: "AgePony",
        tagline: "age encryption with post-quantum recipients.",
        platforms: "iPhone \u{b7} Android \u{b7} macOS \u{b7} Windows \u{b7} Linux",
        url: "https://agepony.com",
        accent: egui::Color32::from_rgb(0x14, 0xB8, 0xB0),
    },
    PonyApp {
        name: "PGPony",
        tagline: "OpenPGP encryption for your messages and files.",
        platforms: "iPhone \u{b7} Android \u{b7} macOS \u{b7} Windows \u{b7} Linux",
        url: "https://pgpony.app",
        accent: egui::Color32::from_rgb(0x5F, 0xFF, 0xAF),
    },
    PonyApp {
        name: "PassPony",
        tagline: "Your pass and passage store, in your pocket.",
        platforms: "iPhone",
        url: "https://passpony.app",
        accent: egui::Color32::from_rgb(0xE8, 0xC8, 0x4D),
    },
    PonyApp {
        name: "QuorumPony",
        tagline: "Split a secret into cards. Any few rebuild it. One alone reveals nothing.",
        platforms: "iPhone",
        url: "https://quorumpony.com",
        accent: egui::Color32::from_rgb(0xC8, 0x97, 0x3A),
    },
    PonyApp {
        name: "ScrubPony",
        tagline: "Strips identifying metadata out of JPEGs without touching a pixel.",
        platforms: "Android \u{b7} macOS \u{b7} Linux",
        url: "https://scrubpony.app",
        accent: egui::Color32::from_rgb(0x9D, 0x7C, 0xF5),
    },
    PonyApp {
        name: "RelayPony",
        tagline: "Encrypted file transfer, phone to phone.",
        platforms: "iPhone \u{b7} Android \u{b7} macOS \u{b7} Windows \u{b7} Linux",
        url: "https://relaypony.app",
        accent: egui::Color32::from_rgb(0x1F, 0x9C, 0xF0),
    },
    PonyApp {
        name: "CarrierPony",
        tagline: "Private messaging and file transfer, sealed end to end.",
        platforms: "iPhone \u{b7} Android",
        url: "https://carrierpony.com",
        accent: egui::Color32::from_rgb(0xF1, 0x66, 0x7B),
    },
    PonyApp {
        name: "BurnPony",
        tagline: "Send a secret. Encrypted on your phone, burned after reading.",
        platforms: "iPhone \u{b7} Android",
        url: "https://burnpony.app",
        accent: egui::Color32::from_rgb(0xF6, 0x75, 0x29),
    },
];

fn family(app: &App, ui: &mut Ui) {
    theme::card(ui, |ui| {
        theme::section(ui, app.t(Key::SecFamily));
        ui.add_space(theme::space::SM);
        for app_link in FAMILY {
            family_row(ui, app_link, app.tr(app_link.tagline));
        }
        ui.add_space(theme::space::SM);
        let every = app.tr("Every Pony app on one page:");
        ui.horizontal(|ui| {
            ui.weak(every);
            ui.hyperlink_to("pony.norsehor.se", "https://pony.norsehor.se");
        });
        ui.add_space(theme::space::TIGHT);
        ui.weak(
            app.tr("Links open in your browser. The app itself never makes a network request."),
        );
    });
}

fn family_row(ui: &mut Ui, app_link: &PonyApp, tagline: &str) {
    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
        ui.painter()
            .circle_filled(dot.center(), 4.0, app_link.accent);
        ui.hyperlink_to(app_link.name, app_link.url);
        ui.label(
            RichText::new(app_link.platforms)
                .font(FontId::new(10.0, egui::FontFamily::Monospace))
                .color(ui.visuals().weak_text_color()),
        );
    });
    ui.horizontal(|ui| {
        ui.add_space(9.0 + theme::space::SM);
        ui.weak(tagline);
    });
    ui.add_space(theme::space::SM);
}

#[cfg(test)]
mod tests {
    use super::{BLOCKS, FAMILY, FAQ, NOT_DOING};
    use crate::i18n::{self, Lang};

    /// Every string routed through `tr` from a const table must actually match a
    /// table entry, or it would silently fall back to English. Proven by
    /// requiring a real German translation (distinct from the English source)
    /// for each, since every entry here carries all six languages.
    #[test]
    fn const_table_strings_are_translated() {
        let mut strings: Vec<&str> = Vec::new();
        for (q, a) in FAQ {
            strings.push(q);
            strings.push(a);
        }
        for (title, body) in BLOCKS {
            strings.push(title);
            strings.push(body);
        }
        strings.extend_from_slice(NOT_DOING);
        for app in FAMILY {
            strings.push(app.tagline);
        }
        for s in strings {
            assert_ne!(
                i18n::tr(Lang::De, s),
                s,
                "no German translation matched (falls back to English): {s:?}"
            );
        }
    }

    #[test]
    fn family_is_the_others_and_only_the_others() {
        let mut seen = std::collections::HashSet::new();
        for a in FAMILY {
            assert!(
                !a.url.contains("vaultpony"),
                "the family list must not contain VaultPony itself"
            );
            assert!(a.url.starts_with("https://"), "{} is not https", a.name);
            assert!(seen.insert(a.url), "{} appears twice", a.name);
        }
        assert_eq!(FAMILY.len(), 8, "eight sibling apps");
    }
}
