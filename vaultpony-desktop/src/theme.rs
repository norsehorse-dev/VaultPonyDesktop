//! The visual system: palette, fonts, scales, icons, and reusable widgets.
//!
//! The brand is taken from the VaultPony app icon: a gold safe on a
//! graphite-navy ground. So the accent is metallic gold, the dark theme is a
//! deep graphite-navy, and the light theme is a warm off-white with navy ink.
//! Fonts are embedded (Inter for UI, JetBrains Mono for paths, Lucide for
//! icons), with a DejaVu Cyrillic fallback so Russian renders too.
//!
//! Both light and dark palettes are registered once; the app switches between
//! them with `ThemeChoice`. Panels build on the helpers here (cards, section
//! and screen headers, styled buttons, the icon rail) rather than raw egui.

use std::sync::Arc;

use egui::{
    Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin,
    Response, RichText, Stroke, Theme, Ui, Vec2, Visuals,
};

// ------------------------------------------------------------------ palette ---

/// Gold accent, the core brand color (from the safe body in the icon).
pub const GOLD: Color32 = Color32::from_rgb(0xCB, 0xA1, 0x4A);
/// Bright gold highlight.
pub const GOLD_LIGHT: Color32 = Color32::from_rgb(0xE8, 0xD0, 0x80);
/// Deep gold, for text/links on light backgrounds where the mid gold is too pale.
pub const GOLD_DEEP: Color32 = Color32::from_rgb(0x9A, 0x77, 0x2C);
/// The safe-door navy, a secondary.
pub const NAVY: Color32 = Color32::from_rgb(0x1B, 0x2A, 0x4A);

/// Danger / destructive.
pub const DANGER: Color32 = Color32::from_rgb(0xD9, 0x53, 0x4F);
pub const DANGER_DEEP: Color32 = Color32::from_rgb(0xA8, 0x35, 0x32);

/// The accent used everywhere. Gold.
pub const ACCENT: Color32 = GOLD;

// -------------------------------------------------------------------- scale ---

pub mod space {
    pub const TIGHT: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const SECTION: f32 = 24.0;
    pub const SCREEN: f32 = 28.0;
}

pub mod radius {
    pub const SM: u8 = 6;
    pub const MD: u8 = 10;
    pub const LG: u8 = 16;
}

const R_BUTTON: u8 = radius::MD;
const R_BLOCK: u8 = radius::MD;

// -------------------------------------------------------------------- fonts ---

pub const UI_FONT: &str = "Inter";
pub const UI_FONT_SEMIBOLD: &str = "InterSemiBold";
pub const MONO_FONT: &str = "JetBrainsMono";
pub const ICON_FONT: &str = "Lucide";
const CYRILLIC: &str = "Cyrillic";

pub fn install(ctx: &Context) {
    install_fonts(ctx);
    ctx.set_visuals_of(Theme::Dark, palette(true));
    ctx.set_visuals_of(Theme::Light, palette(false));
    install_text_styles(ctx);
}

fn install_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    };
    add(
        UI_FONT,
        include_bytes!("../assets/fonts/Inter-Regular-subset.ttf"),
    );
    add(
        UI_FONT_SEMIBOLD,
        include_bytes!("../assets/fonts/Inter-SemiBold-subset.ttf"),
    );
    add(
        MONO_FONT,
        include_bytes!("../assets/fonts/JetBrainsMono-Regular-subset.ttf"),
    );
    add(
        ICON_FONT,
        include_bytes!("../assets/fonts/Lucide-subset.ttf"),
    );
    add(
        CYRILLIC,
        include_bytes!("../assets/fonts/DejaVuSans-Cyrillic-subset.ttf"),
    );

    // Proportional: Inter first, Cyrillic fallback for glyphs Inter lacks.
    if let Some(p) = fonts.families.get_mut(&FontFamily::Proportional) {
        p.insert(0, UI_FONT.to_owned());
        p.push(CYRILLIC.to_owned());
    }
    if let Some(m) = fonts.families.get_mut(&FontFamily::Monospace) {
        m.insert(0, MONO_FONT.to_owned());
        m.push(CYRILLIC.to_owned());
    }
    fonts.families.insert(
        FontFamily::Name(UI_FONT_SEMIBOLD.into()),
        vec![UI_FONT_SEMIBOLD.to_owned(), CYRILLIC.to_owned()],
    );
    fonts.families.insert(
        FontFamily::Name(ICON_FONT.into()),
        vec![ICON_FONT.to_owned()],
    );

    ctx.set_fonts(fonts);
}

fn install_text_styles(ctx: &Context) {
    use egui::{FontFamily::Proportional, TextStyle};
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, semibold(19.0)),
            (TextStyle::Body, FontId::new(14.0, Proportional)),
            (TextStyle::Button, semibold(14.0)),
            (TextStyle::Small, FontId::new(11.5, Proportional)),
            (
                TextStyle::Monospace,
                FontId::new(12.5, FontFamily::Monospace),
            ),
        ]
        .into();
        style.spacing.item_spacing = Vec2::new(space::SM, space::SM);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        style.spacing.window_margin = Margin::same(space::MD as i8);
    });
}

/// A semibold Inter font at `size`.
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(UI_FONT_SEMIBOLD.into()))
}

fn palette(dark: bool) -> Visuals {
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    if dark {
        v.panel_fill = Color32::from_rgb(0x0D, 0x13, 0x1B);
        v.window_fill = Color32::from_rgb(0x12, 0x1A, 0x24);
        v.extreme_bg_color = Color32::from_rgb(0x08, 0x0D, 0x13);
        v.faint_bg_color = Color32::from_rgb(0x18, 0x22, 0x2E);
        v.window_stroke.color = Color32::from_rgb(0x2A, 0x37, 0x47);
        v.override_text_color = Some(Color32::from_rgb(0xE7, 0xE3, 0xD8));
    } else {
        v.panel_fill = Color32::from_rgb(0xF6, 0xF3, 0xEC);
        v.window_fill = Color32::from_rgb(0xFC, 0xFA, 0xF5);
        v.extreme_bg_color = Color32::from_rgb(0xFF, 0xFF, 0xFF);
        v.faint_bg_color = Color32::from_rgb(0xEF, 0xEA, 0xDE);
        v.window_stroke.color = Color32::from_rgb(0xD9, 0xD0, 0xBC);
        v.override_text_color = Some(Color32::from_rgb(0x22, 0x2A, 0x36));
    }

    let radius = CornerRadius::same(radius::SM);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = radius;
    }

    v.hyperlink_color = if dark { GOLD_LIGHT } else { GOLD_DEEP };
    v.selection.bg_fill = ACCENT.gamma_multiply(if dark { 0.32 } else { 0.22 });
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v
}

/// The primary text ink for the current theme.
pub fn ink(ui: &Ui) -> Color32 {
    ui.visuals()
        .override_text_color
        .unwrap_or_else(|| ui.visuals().text_color())
}

/// The fill for a card, a touch lifted from the panel.
pub fn card_fill(ui: &Ui) -> Color32 {
    ui.visuals().faint_bg_color
}

/// A deeper danger color that reads on both themes.
pub fn danger_ink(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        DANGER
    } else {
        DANGER_DEEP
    }
}

// -------------------------------------------------------------------- icons ---

/// Lucide glyphs available in the embedded subset.
pub mod ic {
    pub const FILES: char = '\u{E0CF}';
    pub const KEY_ROUND: char = '\u{E4A3}';
    pub const SETTINGS: char = '\u{E154}';
    pub const UPLOAD: char = '\u{E19E}';
    pub const DOWNLOAD: char = '\u{E0B2}';
    pub const PLUS: char = '\u{E13D}';
    pub const ARROW_RIGHT: char = '\u{E049}';
    pub const X: char = '\u{E1B2}';
    pub const CHECK: char = '\u{E06C}';
    pub const TRASH: char = '\u{E18E}';
    pub const PENCIL: char = '\u{E1F9}';
    pub const LOCK: char = '\u{E10B}';
    pub const LOCK_OPEN: char = '\u{E10C}';
    pub const CIRCLE_CHECK: char = '\u{E226}';
    pub const CIRCLE_ALERT: char = '\u{E077}';
    pub const FILE_LOCK: char = '\u{E31E}';
}

pub fn icon_text(glyph: char, size: f32) -> RichText {
    RichText::new(glyph).font(FontId::new(size, FontFamily::Name(ICON_FONT.into())))
}

pub fn icon(ui: &mut Ui, glyph: char, size: f32, color: Color32) {
    ui.label(icon_text(glyph, size).color(color));
}

// ------------------------------------------------------------------ widgets ---

/// A screen title with subtitle and optional right-aligned actions.
pub fn screen_head(ui: &mut Ui, title: &str, subtitle: &str, actions: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).font(semibold(22.0)).color(ink(ui)));
            if !subtitle.is_empty() {
                ui.add_space(space::TIGHT);
                ui.label(
                    RichText::new(subtitle)
                        .font(FontId::proportional(12.5))
                        .color(ui.visuals().weak_text_color()),
                );
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), actions);
    });
    ui.add_space(space::MD);
}

/// A section label inside a card.
pub fn section(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .font(semibold(11.0))
            .color(ACCENT.gamma_multiply(if ui.visuals().dark_mode { 0.95 } else { 0.85 })),
    );
}

/// A rounded content card.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let stroke = Stroke::new(1.0, ui.visuals().window_stroke.color);
    egui::Frame::new()
        .fill(card_fill(ui))
        .corner_radius(CornerRadius::same(R_BLOCK))
        .inner_margin(Margin::same(space::LG as i8))
        .stroke(stroke)
        .show(ui, |ui| {
            ui.set_width(ui.available_width() - 2.0 * space::LG);
            add(ui)
        })
        .inner
}

/// A filled gold primary button. Dark ink on gold reads on both themes.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    primary_button_enabled(ui, text, true)
}

pub fn primary_button_enabled(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let btn = egui::Button::new(
        RichText::new(text)
            .font(semibold(14.0))
            .color(Color32::from_rgb(0x14, 0x18, 0x1E)),
    )
    .fill(if enabled {
        ACCENT
    } else {
        ACCENT.gamma_multiply(0.4)
    })
    .corner_radius(CornerRadius::same(R_BUTTON));
    ui.add_enabled(enabled, btn)
}

/// An outline secondary button.
pub fn secondary_button(ui: &mut Ui, text: &str) -> Response {
    let btn = egui::Button::new(RichText::new(text).font(semibold(14.0)).color(ink(ui)))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0, ui.visuals().window_stroke.color))
        .corner_radius(CornerRadius::same(R_BUTTON));
    ui.add(btn)
}

/// A destructive (red) button.
pub fn destructive_button_enabled(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let btn = egui::Button::new(
        RichText::new(text)
            .font(semibold(14.0))
            .color(Color32::WHITE),
    )
    .fill(if enabled {
        DANGER
    } else {
        DANGER.gamma_multiply(0.4)
    })
    .corner_radius(CornerRadius::same(R_BUTTON));
    ui.add_enabled(enabled, btn)
}

/// A segmented control. Returns the newly-selected index if it changed.
pub fn segmented(ui: &mut Ui, labels: &[&str], selected: usize) -> Option<usize> {
    let mut changed = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = space::TIGHT;
        for (i, label) in labels.iter().enumerate() {
            let is_sel = i == selected;
            let text = RichText::new(*label).font(semibold(13.0)).color(if is_sel {
                Color32::from_rgb(0x14, 0x18, 0x1E)
            } else {
                ink(ui)
            });
            let btn = egui::Button::new(text)
                .fill(if is_sel {
                    ACCENT
                } else {
                    ui.visuals().faint_bg_color
                })
                .corner_radius(CornerRadius::same(radius::SM));
            if ui.add(btn).clicked() && !is_sel {
                changed = Some(i);
            }
        }
    });
    changed
}

/// The brand header at the top of the rail: the mark and the wordmark.
pub fn rail_head(ui: &mut Ui) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::hover());
        draw_mark(ui, rect);
        ui.add_space(space::TIGHT);
        ui.label(
            RichText::new("VaultPony")
                .font(semibold(17.0))
                .color(ink(ui)),
        );
    });
}

/// A rail destination: icon, label, and a selected pill.
pub fn rail_item(ui: &mut Ui, glyph: char, label: &str, selected: bool) -> Response {
    let fill = if selected {
        ACCENT.gamma_multiply(if ui.visuals().dark_mode { 0.20 } else { 0.16 })
    } else {
        Color32::TRANSPARENT
    };
    let text_color = if selected { ACCENT } else { ink(ui) };
    let resp = egui::Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(radius::SM))
        .inner_margin(Margin::symmetric(space::SM as i8, 7))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());
                icon(ui, glyph, 15.0, text_color);
                ui.add_space(space::TIGHT);
                ui.label(RichText::new(label).font(semibold(13.5)).color(text_color));
            });
        })
        .response;
    resp.interact(egui::Sense::click())
}

/// A one-line status pill under a form.
pub fn status_line(ui: &mut Ui, message: &str, error: bool) {
    if message.is_empty() {
        return;
    }
    let (glyph, color) = if error {
        (ic::CIRCLE_ALERT, danger_ink(ui))
    } else {
        (ic::CIRCLE_CHECK, ACCENT)
    };
    ui.horizontal(|ui| {
        icon(ui, glyph, 14.0, color);
        ui.add_space(space::TIGHT);
        ui.label(RichText::new(message).color(color));
    });
}

/// An empty-state message centered in the available space.
pub fn empty_state(ui: &mut Ui, glyph: char, message: &str) {
    ui.add_space(space::SECTION);
    ui.vertical_centered(|ui| {
        icon(ui, glyph, 34.0, ui.visuals().weak_text_color());
        ui.add_space(space::SM);
        ui.label(
            RichText::new(message)
                .font(FontId::proportional(13.0))
                .color(ui.visuals().weak_text_color()),
        );
    });
}

/// The VaultPony mark: a gold safe plate with a keyhole, drawn to `rect`.
pub fn draw_mark(ui: &Ui, rect: egui::Rect) {
    let p = ui.painter();
    let plate = rect.shrink(rect.width() * 0.04);
    p.rect_filled(plate, CornerRadius::same(4), GOLD_DEEP);
    let door = plate.shrink(rect.width() * 0.16);
    p.rect_filled(door, CornerRadius::same(3), NAVY);
    // Dial ring.
    let dial = door.left_center() + Vec2::new(door.width() * 0.30, 0.0);
    p.circle_stroke(dial, rect.width() * 0.11, Stroke::new(1.6, GOLD_LIGHT));
    p.circle_filled(dial, rect.width() * 0.045, GOLD);
}
