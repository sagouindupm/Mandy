//! Fingerprints the shader translator, so that cached translations
//! (`src/shader_cache.rs`) last until the translator itself changes and not
//! only until the next build of anything; and compiles the mesh launch
//! stage (`src/launch.hlsl`, see `src/launch.rs`).

use std::path::{Path, PathBuf};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn main() {
    let here = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    let crates = here.join("..");
    let root = crates.join("..");
    // The translator, what it is built from, and the form entries are kept in.
    let mut all = vec![
        crates.join("victory-shader/Cargo.toml"),
        here.join("src/shader_cache.rs"),
        root.join("Cargo.lock"),
        root.join("rust-toolchain.toml"),
    ];
    let source = crates.join("victory-shader/src");
    println!("cargo:rerun-if-changed={}", source.display());
    files(&source, &mut all);
    all.sort();
    let mut hash = blake3::Hasher::new();
    for path in &all {
        println!("cargo:rerun-if-changed={}", path.display());
        // Names and line endings as they are in every checkout.
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let bytes: Vec<u8> = std::fs::read(path).unwrap_or_default().into_iter().filter(|&b| b != 13).collect();
        hash.update(name.as_bytes());
        hash.update(&(bytes.len() as u64).to_le_bytes());
        hash.update(&bytes);
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-env=VICTORY_TRANSLATOR={}", hash.finalize().to_hex());
    launch(&here);
}

/// The PC `dxc.exe`: `VICTORY_PC_DXC`, else the newest Windows SDK's (which
/// signs what it compiles with the `dxil.dll` beside it).
fn dxc() -> Option<PathBuf> {
    println!("cargo:rerun-if-env-changed=VICTORY_PC_DXC");
    if let Some(path) = std::env::var_os("VICTORY_PC_DXC") {
        return Some(PathBuf::from(path));
    }
    let kits = PathBuf::from(std::env::var_os("ProgramFiles(x86)")?).join("Windows Kits/10/bin");
    let mut versions: Vec<(Vec<u32>, PathBuf)> = std::fs::read_dir(&kits)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let version = name.split('.').map(str::parse).collect::<Result<Vec<u32>, _>>().ok()?;
            let dxc = entry.path().join("x64/dxc.exe");
            dxc.exists().then_some((version, dxc))
        })
        .collect();
    versions.sort();
    versions.pop().map(|(_, dxc)| dxc)
}

/// `launch.dxil` in `OUT_DIR`: the amplification stage of `src/launch.rs`,
/// compiled here from its source by the SDK's compiler (no compiled bytes
/// are kept in the tree). Empty, with a warning, where there is no
/// compiler: the mesh draws then go without the stage, as with
/// `VICTORY_MESH_LAUNCH=0`.
fn launch(here: &Path) {
    let source = here.join("src/launch.hlsl");
    println!("cargo:rerun-if-changed={}", source.display());
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default()).join("launch.dxil");
    let _ = std::fs::remove_file(&out);
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    let compiled = windows
        && match dxc() {
            None => {
                println!("cargo:warning=no PC dxc.exe (VICTORY_PC_DXC, or a Windows SDK): mesh draws go without the launch stage");
                false
            }
            Some(dxc) => {
                let status = std::process::Command::new(&dxc)
                    .args(["-nologo", "-T", "as_6_6", "-E", "main", "-Fo"])
                    .arg(&out)
                    .arg(&source)
                    .status();
                match status {
                    Ok(s) if s.success() => true,
                    Ok(s) => {
                        println!("cargo:warning=the mesh launch stage did not compile ({} {s}): mesh draws go without it", dxc.display());
                        false
                    }
                    Err(e) => {
                        println!("cargo:warning=no mesh launch stage ({} {e}): mesh draws go without it", dxc.display());
                        false
                    }
                }
            }
        };
    if !compiled {
        std::fs::write(&out, []).expect("OUT_DIR is writable");
    }
}
