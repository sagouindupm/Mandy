// The app icon (window, title bar, taskbar, exe) is drawn here and written to icons/icon.ico before
// tauri-build reads it: the repository only holds text files (docs/asset-policy.md), so no binary icon is
// committed. Same geometry as the window's logo (src/components/logo.tsx), on a 64 x 64 grid: a rounded
// square (corner radius 14) in #FF3C78 and a geometric « S » in #0B0B10, two bowls of radius 9 centred at
// (32, 23) and (32, 41), stroke 8, square ends.
use std::path::Path;

fn main() {
    let ico = Path::new(env!("CARGO_MANIFEST_DIR")).join("icons").join("icon.ico");
    let bytes = make_ico(&[16, 24, 32, 48, 64, 256]);
    // Written only when it changes, so an unchanged icon does not rebuild the app.
    if std::fs::read(&ico).ok().as_deref() != Some(bytes.as_slice()) {
        std::fs::create_dir_all(ico.parent().unwrap()).unwrap();
        std::fs::write(&ico, bytes).unwrap();
    }
    println!("cargo:rerun-if-changed=build.rs");
    tauri_build::build()
}

/// Whether a point (64 x 64 grid, y down) is in the rounded square, and in the S.
fn shape(x: f32, y: f32) -> (bool, bool) {
    let r = 14.0;
    let (cx, cy) = (x.clamp(r, 64.0 - r), y.clamp(r, 64.0 - r));
    let square = (0.0..=64.0).contains(&x) && (0.0..=64.0).contains(&y) && (x - cx).powi(2) + (y - cy).powi(2) <= r * r;
    // A ring of radius 9 and half-width 4, kept between two angles (degrees, y down, 0 = right).
    let arc = |ox: f32, oy: f32, from: f32, to: f32| {
        let d = ((x - ox).powi(2) + (y - oy).powi(2)).sqrt();
        let mut a = (y - oy).atan2(x - ox).to_degrees();
        if a < from {
            a += 360.0;
        }
        (5.0..=13.0).contains(&d) && a <= to
    };
    // Top bowl: from the upper-right end (-30 degrees) back over the top and the left to the bottom (90),
    // i.e. 90..330. Bottom bowl: from its top (-90) over the right to the lower-left end (150).
    let s = arc(32.0, 23.0, 90.0, 330.0) || arc(32.0, 41.0, -90.0, 150.0);
    (square, s)
}

fn image(size: u32) -> Vec<u8> {
    const SS: u32 = 4;
    let (pink, ink) = ([0xFFu8, 0x3C, 0x78], [0x0Bu8, 0x0B, 0x10]);
    let mut bgra = Vec::with_capacity((size * size * 4) as usize);
    // Rows bottom-up, as an ICO bitmap wants.
    for row in (0..size).rev() {
        for col in 0..size {
            let (mut cover, mut mark) = (0.0f32, 0.0f32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (col as f32 + (sx as f32 + 0.5) / SS as f32) * 64.0 / size as f32;
                    let y = (row as f32 + (sy as f32 + 0.5) / SS as f32) * 64.0 / size as f32;
                    let (inside, s) = shape(x, y);
                    if inside {
                        cover += 1.0;
                        if s {
                            mark += 1.0;
                        }
                    }
                }
            }
            let n = (SS * SS) as f32;
            let (a, m) = (cover / n, if cover > 0.0 { mark / cover } else { 0.0 });
            let c = |i: usize| (pink[i] as f32 + (ink[i] as f32 - pink[i] as f32) * m).round() as u8;
            bgra.extend_from_slice(&[c(2), c(1), c(0), (a * 255.0).round() as u8]);
        }
    }
    bgra
}

fn make_ico(sizes: &[u32]) -> Vec<u8> {
    let mut entries = Vec::new();
    for &s in sizes {
        let mut bmp = Vec::new();
        let mask_row = s.div_ceil(32) * 4;
        let pixels = image(s);
        for v in [40u32, s, s * 2] {
            bmp.extend_from_slice(&v.to_le_bytes());
        }
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&32u16.to_le_bytes());
        for v in [0u32, pixels.len() as u32 + mask_row * s, 0, 0, 0, 0] {
            bmp.extend_from_slice(&v.to_le_bytes());
        }
        bmp.extend_from_slice(&pixels);
        bmp.extend(std::iter::repeat_n(0u8, (mask_row * s) as usize));
        entries.push((s, bmp));
    }
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * entries.len() as u32;
    for (s, bmp) in &entries {
        let dim = if *s >= 256 { 0 } else { *s as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(bmp.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += bmp.len() as u32;
    }
    for (_, bmp) in entries {
        out.extend_from_slice(&bmp);
    }
    out
}
