//! Where things are: the repository (victory.exe, series-s/reglages.json) and the launcher's own folder
//! %LOCALAPPDATA%\Sagouindupm (lanceur.json, victory.toml, journaux, captures), the same as series-s/reglages.ps1.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    pub root: PathBuf,
    pub exe: PathBuf,
    pub reglages: PathBuf,
    pub home: PathBuf,
    pub conf: PathBuf,
    pub toml: PathBuf,
    pub logs: PathBuf,
    /// The player's own screenshots (backgrounds of the window, « Capturer maintenant »).
    pub captures: PathBuf,
}

impl Paths {
    pub fn discover() -> Self {
        let root = find_root();
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let home = local.join("Sagouindupm");
        let logs = home.join("journaux");
        let _ = std::fs::create_dir_all(&logs);
        Paths {
            exe: root.join("target").join("release").join("victory.exe"),
            reglages: root.join("series-s").join("reglages.json"),
            conf: home.join("lanceur.json"),
            captures: home.join("captures"),
            toml: home.join("victory.toml"),
            root,
            home,
            logs,
        }
    }
}

fn is_root(dir: &Path) -> bool {
    dir.join(".victory-root").is_file() && dir.join("series-s").join("reglages.json").is_file()
}

/// The repository: `--root <dir>` or SAGOUINDUPM_ROOT (set by Sagouindupm-Launcher.bat), else the first folder
/// above the exe or the current folder that holds .victory-root, else where the launcher was compiled from.
fn find_root() -> PathBuf {
    let mut args = std::env::args_os();
    while let Some(a) = args.next() {
        if a == "--root"
            && let Some(dir) = args.next()
        {
            let dir = PathBuf::from(dir);
            if is_root(&dir) {
                return dir;
            }
        }
    }
    if let Some(dir) = std::env::var_os("SAGOUINDUPM_ROOT").map(PathBuf::from)
        && is_root(&dir)
    {
        return dir;
    }
    let starts = [std::env::current_exe().ok(), std::env::current_dir().ok()];
    for start in starts.into_iter().flatten() {
        if let Some(found) = start.ancestors().find(|d| is_root(d)) {
            return found.to_path_buf();
        }
    }
    // launcher/src-tauri -> the repository.
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").canonicalize().unwrap_or_else(|_| PathBuf::from("."))
}

/// victory.exe's cache folder (crates/victory-core/src/config.rs): VICTORY_CACHE_DIR, else %LOCALAPPDATA%\victory\cache.
pub fn victory_dir(var: &str, name: &str) -> PathBuf {
    if let Some(v) = std::env::var_os(var).filter(|v| !v.is_empty()) {
        return PathBuf::from(v);
    }
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default().join("victory").join(name)
}

/// A program in PATH.
pub fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(name)).find(|p| p.is_file())
}
