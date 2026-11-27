// The app icon (window, title bar, taskbar, exe) is rendered here from the mascot's SVG drawings and written to
// icons/icon.ico before tauri-build reads it: the repository only holds text files (docs/asset-policy.md), so
// no binary icon is committed. The drawings are the window's logo (src/components/logo.tsx):
// ../src/assets/sagouin-small.svg for 16 to 32 px (face silhouette, ear tufts, blaze and eyes, what still reads
// there) and ../src/assets/sagouin.svg from 48 px. Rendered with resvg (a build-dependency only; no fonts or
// raster images in the drawings, so its default features are off).
use std::path::Path;

use resvg::{tiny_skia, usvg};

/// The sizes in the .ico; the simplified drawing below `DETAILED_FROM`.
const SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];
const DETAILED_FROM: u32 = 48;

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let assets = dir.join("..").join("src").join("assets");
    let (small, detailed) = (assets.join("sagouin-small.svg"), assets.join("sagouin.svg"));
    let (small_tree, detailed_tree) = (load(&small), load(&detailed));
    let images: Vec<(u32, Vec<u8>)> = SIZES
        .iter()
        .map(|&s| (s, render(if s >= DETAILED_FROM { &detailed_tree } else { &small_tree }, s)))
        .collect();
    let bytes = make_ico(&images);
    let ico = dir.join("icons").join("icon.ico");
    // Written only when it changes, so an unchanged icon does not rebuild the app.
    if std::fs::read(&ico).ok().as_deref() != Some(bytes.as_slice()) {
        std::fs::create_dir_all(ico.parent().unwrap()).unwrap();
        std::fs::write(&ico, bytes).unwrap();
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", small.display());
    println!("cargo:rerun-if-changed={}", detailed.display());
    tauri_build::build()
}

fn load(path: &Path) -> usvg::Tree {
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    usvg::Tree::from_data(&data, &usvg::Options::default()).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The drawing scaled to `size` x `size`: straight (not premultiplied) BGRA, rows bottom-up, as an ICO bitmap
/// wants.
fn render(tree: &usvg::Tree, size: u32) -> Vec<u8> {
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("icon size");
    let scale = size as f32 / tree.size().width().max(tree.size().height());
    resvg::render(tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let (n, px) = (size as usize, pixmap.pixels());
    let mut bgra = Vec::with_capacity(n * n * 4);
    for row in (0..n).rev() {
        for p in &px[row * n..(row + 1) * n] {
            let c = p.demultiply();
            bgra.extend_from_slice(&[c.blue(), c.green(), c.red(), c.alpha()]);
        }
    }
    bgra
}

/// An .ico of 32-bit BMP entries, from (size, BGRA bottom-up) images.
fn make_ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut entries = Vec::new();
    for (s, pixels) in images {
        let (s, mut bmp) = (*s, Vec::new());
        let mask_row = s.div_ceil(32) * 4;
        for v in [40u32, s, s * 2] {
            bmp.extend_from_slice(&v.to_le_bytes());
        }
        bmp.extend_from_slice(&1u16.to_le_bytes());
        bmp.extend_from_slice(&32u16.to_le_bytes());
        for v in [0u32, pixels.len() as u32 + mask_row * s, 0, 0, 0, 0] {
            bmp.extend_from_slice(&v.to_le_bytes());
        }
        bmp.extend_from_slice(pixels);
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
