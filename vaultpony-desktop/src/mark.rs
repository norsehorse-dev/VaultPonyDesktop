//! The app icon: the real VaultPony mark (a gold safe on graphite), decoded
//! from the embedded PNG and composited over the graphite ground. Used for the
//! window/dock icon and the mark in the rail.

const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

/// Graphite gradient the adaptive icon uses: top to bottom.
const BG_TOP: (u8, u8, u8) = (0x17, 0x21, 0x2F);
const BG_BOTTOM: (u8, u8, u8) = (0x0A, 0x0F, 0x17);

/// Composite the icon foreground over the graphite ground at `size` x `size`.
/// When `round`, the corners are cut to a rounded square (for the in-app tile);
/// the window/dock icon is left square so the OS applies its own mask.
fn composite(size: u32, round: bool) -> Vec<u8> {
    let fg = image::load_from_memory(ICON_PNG)
        .expect("embedded icon.png decodes")
        .to_rgba8();
    let fg = image::imageops::resize(&fg, size, size, image::imageops::FilterType::Lanczos3);

    let n = size.max(1) as f32;
    let corner = size as f32 * 0.22;
    let inner = size as f32 - corner;

    let mut out = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        let t = y as f32 / (n - 1.0).max(1.0);
        let bg = lerp(BG_TOP, BG_BOTTOM, t);
        for x in 0..size {
            let p = fg.get_pixel(x, y);
            let a = p[3] as f32 / 255.0;
            let r = (p[0] as f32 * a + bg.0 as f32 * (1.0 - a)) as u8;
            let g = (p[1] as f32 * a + bg.1 as f32 * (1.0 - a)) as u8;
            let b = (p[2] as f32 * a + bg.2 as f32 * (1.0 - a)) as u8;
            let alpha = if round && outside_round(x as f32, y as f32, size as f32, corner, inner) {
                0
            } else {
                255
            };
            let i = ((y * size + x) * 4) as usize;
            out[i] = r;
            out[i + 1] = g;
            out[i + 2] = b;
            out[i + 3] = alpha;
        }
    }
    out
}

fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// Is `(x, y)` outside the rounded-square of corner radius `corner`?
fn outside_round(x: f32, y: f32, size: f32, corner: f32, inner: f32) -> bool {
    let cx = if x < corner {
        corner
    } else if x > inner {
        inner
    } else {
        x
    };
    let cy = if y < corner {
        corner
    } else if y > inner {
        inner
    } else {
        y
    };
    let _ = size;
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy > corner * corner
}

/// The window and dock icon (left square for the OS mask).
pub fn window_icon() -> egui::IconData {
    const S: u32 = 256;
    egui::IconData {
        rgba: composite(S, false),
        width: S,
        height: S,
    }
}

/// A rounded icon tile for the rail, as an egui image.
pub fn rail_image(size: u32) -> egui::ColorImage {
    let rgba = composite(size, true);
    egui::ColorImage::from_rgba_unmultiplied([size as usize, size as usize], &rgba)
}
