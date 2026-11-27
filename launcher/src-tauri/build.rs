// The app icon is drawn here (a rounded square in the launcher's pink with a white "play" triangle) and
// written to icons/icon.ico before tauri-build reads it: the repository only holds text files
// (docs/asset-policy.md), so no binary icon is committed.
use std::path::Path;

fn main() {
    let ico = Path::new(env!("CARGO_MANIFEST_DIR")).join("icons").join("icon.ico");
    if !ico.exists() {
        std::fs::create_dir_all(ico.parent().unwrap()).unwrap();
        std::fs::write(&ico, make_ico(&[16, 24, 32, 48, 64, 256])).unwrap();
    }
    println!("cargo:rerun-if-changed=build.rs");
    tauri_build::build()
}

/// At a point of the unit square: coverage of the rounded square (0 or 1), and whether it is in the triangle.
fn shape(x: f32, y: f32) -> (f32, bool) {
    let r = 0.22;
    let (cx, cy) = (x.clamp(r, 1.0 - r), y.clamp(r, 1.0 - r));
    let inside = (x - cx).powi(2) + (y - cy).powi(2) <= r * r;
    // Play triangle pointing right, centred optically: left edge at x = 0.39, tip at x = 0.73.
    let (left, tip, half_height) = (0.39, 0.73, 0.21);
    let tri = x >= left && {
        let t = (x - left) / (tip - left);
        t <= 1.0 && (y - 0.5).abs() <= half_height * (1.0 - t)
    };
    (if inside { 1.0 } else { 0.0 }, tri)
}

fn image(size: u32) -> Vec<u8> {
    const SS: u32 = 4;
    let mut bgra = Vec::with_capacity((size * size * 4) as usize);
    // Rows bottom-up, as an ICO bitmap wants.
    for row in (0..size).rev() {
        for col in 0..size {
            let (mut cover, mut white) = (0.0f32, 0.0f32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = (col as f32 + (sx as f32 + 0.5) / SS as f32) / size as f32;
                    let y = (row as f32 + (sy as f32 + 0.5) / SS as f32) / size as f32;
                    let (c, tri) = shape(x, y);
                    cover += c;
                    if c > 0.0 && tri {
                        white += 1.0;
                    }
                }
            }
            let n = (SS * SS) as f32;
            let (a, w) = (cover / n, if cover > 0.0 { white / cover } else { 0.0 });
            // #FF3C78 mixed with white by w.
            let mix = |c: f32| (c + (255.0 - c) * w).round() as u8;
            bgra.extend_from_slice(&[mix(0x78 as f32), mix(0x3C as f32), mix(0xFF as f32), (a * 255.0).round() as u8]);
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
