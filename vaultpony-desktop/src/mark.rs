//! The app mark, rasterized procedurally into RGBA for the window and dock icon.
//!
//! Placeholder art: a rounded steel plate with a circular vault door and a
//! keyhole. Dependency-free and deterministic. A real VaultPony mark (drawn from
//! the same vertex data the UI uses, the way AgePony does it) is a later design
//! task; this keeps the window from shipping with a blank icon in the meantime.

/// Rasterize the mark at `size` x `size`, returning `size * size * 4` RGBA bytes.
pub fn rasterise(size: usize) -> Vec<u8> {
    let n = size as f32;
    let mut px = vec![0u8; size * size * 4];

    let plate = (0x2b, 0x64, 0xd6); // steel blue
    let door = (0x14, 0x2c, 0x5e); // deeper blue
    let ring = (0x8f, 0xb4, 0xf5); // light rim
    let hole = (0x0a, 0x0c, 0x10); // near-black keyhole

    let cx = n / 2.0;
    let cy = n / 2.0;
    let plate_r = n * 0.42;
    let corner = n * 0.16;
    let half = n * 0.34; // half-extent of the rounded square plate
    let door_r = n * 0.30;
    let ring_r = n * 0.335;

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let dx = fx - cx;
            let dy = fy - cy;

            let mut color: Option<(u8, u8, u8)> = None;

            // Rounded-square plate.
            if rounded_square(dx, dy, half, corner) {
                color = Some(plate);
            }

            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= ring_r && dist > door_r {
                color = Some(ring);
            }
            if dist <= door_r {
                color = Some(door);
            }

            // Keyhole: a small circle over a tapered slot, centered.
            let hole_top = cy - n * 0.02;
            let khx = fx - cx;
            let khy = fy - hole_top;
            let circle = khx * khx + khy * khy <= (n * 0.055).powi(2);
            let slot = khy > 0.0 && khy < n * 0.14 && khx.abs() < n * 0.028 + khy * 0.10;
            if dist <= door_r && (circle || slot) {
                color = Some(hole);
            }

            let _ = plate_r;
            if let Some((r, g, b)) = color {
                let i = (y * size + x) * 4;
                px[i] = r;
                px[i + 1] = g;
                px[i + 2] = b;
                px[i + 3] = 0xff;
            }
        }
    }
    px
}

/// Is `(dx, dy)` inside a square of half-extent `half` with rounded corners of
/// radius `corner`, measured from the square's center?
fn rounded_square(dx: f32, dy: f32, half: f32, corner: f32) -> bool {
    let ax = dx.abs();
    let ay = dy.abs();
    if ax > half || ay > half {
        return false;
    }
    let inner = half - corner;
    if ax <= inner || ay <= inner {
        return true;
    }
    let ox = ax - inner;
    let oy = ay - inner;
    ox * ox + oy * oy <= corner * corner
}
