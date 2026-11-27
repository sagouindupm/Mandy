//! lanceur.json (the player's choices, shared with the console launcher) and series-s/reglages.json (the
//! Series S settings, the one source also read by series-s/reglages.ps1). `game_env` applies them exactly as
//! Set-GameEnv in reglages.ps1 does.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::paths::Paths;
pub use crate::profile::LowRam;
use crate::profile::{Hardware, Materiel, Profile, put};

// ------------------------------------------------------------------ lanceur.json

/// lanceur.json as a JSON object: keys this launcher does not know are kept as they are.
pub struct Settings(pub Map<String, Value>);

/// Keys of lanceur.json, with their default when an older file lacks them (as Complete-Settings in reglages.ps1).
/// pipelines_when (this launcher only): Unix time of the last successful « victory warm --used ».
fn defaults() -> [(&'static str, Value); 9] {
    [
        ("game_exe", json!("")),
        ("shaders_dir", json!("")),
        ("shaders_ready", json!(false)),
        ("parties", json!(0)),
        ("dlss_dir", json!("")),
        ("dlss_mode", json!("")),
        ("pipelines_parties", json!(-1)),
        ("nv_cache_ok", json!(false)),
        ("ancien_plan", json!("")),
    ]
}

impl Settings {
    pub fn load(paths: &Paths) -> Self {
        let mut map = std::fs::read(&paths.conf)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(strip_bom(&b)).ok())
            .and_then(|v| match v {
                Value::Object(m) => Some(m),
                _ => None,
            })
            .unwrap_or_default();
        for (k, def) in defaults() {
            map.entry(k.to_string()).or_insert(def);
        }
        Settings(map)
    }

    /// Written in UTF-8 with a BOM, like Windows PowerShell's Set-Content -Encoding utf8 (which reads it back).
    pub fn save(&self, paths: &Paths) -> Result<(), String> {
        let text = serde_json::to_string_pretty(&Value::Object(self.0.clone())).map_err(|e| e.to_string())?;
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(text.replace('\n', "\r\n").as_bytes());
        bytes.extend_from_slice(b"\r\n");
        std::fs::write(&paths.conf, bytes).map_err(|e| format!("{}: {e}", paths.conf.display()))
    }

    pub fn str(&self, k: &str) -> String {
        match self.0.get(k) {
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        }
    }
    pub fn int(&self, k: &str) -> i64 {
        match self.0.get(k) {
            Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
            Some(Value::String(s)) => s.trim().parse().unwrap_or(0),
            _ => 0,
        }
    }
    pub fn bool(&self, k: &str) -> bool {
        matches!(self.0.get(k), Some(Value::Bool(true)))
    }
    pub fn set(&mut self, k: &str, v: impl Into<Value>) {
        self.0.insert(k.to_string(), v.into());
    }

    pub fn game_ok(&self) -> bool {
        let g = self.str("game_exe");
        !g.is_empty() && Path::new(&g).is_file()
    }
}

fn strip_bom(b: &[u8]) -> &[u8] {
    b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b)
}

/// victory.toml: the game's folder, read by victory.exe through VICTORY_CONFIG (as Write-VictoryToml).
pub fn write_victory_toml(paths: &Paths, game_exe: &str) -> Result<(), String> {
    let dir = Path::new(game_exe).parent().map(|p| p.display().to_string()).unwrap_or_default();
    // A TOML literal string cannot hold a quote: then a basic string, escaped.
    let value = if dir.contains('\'') {
        format!("\"{}\"", dir.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        format!("'{dir}'")
    };
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(format!("[paths]\ngame_dir = {value}\ntest_titles = []\n\r\n").as_bytes());
    std::fs::write(&paths.toml, bytes).map_err(|e| format!("{}: {e}", paths.toml.display()))
}

// ------------------------------------------------------------------ reglages.json

#[derive(Deserialize, Serialize, Clone)]
pub struct Group {
    pub titre: String,
    pub note: String,
    pub env: Map<String, Value>,
}

#[derive(Deserialize, Clone)]
pub struct Skip {
    pub variable: String,
    pub fichier: String,
    pub en_plus: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct DlssMode {
    pub id: String,
    pub nom: String,
    pub mesure: String,
}

#[derive(Deserialize, Clone)]
pub struct Dlss {
    pub note: String,
    pub fichier_requis: String,
    pub variable_dossier: String,
    pub variable_mode: String,
    pub retire: Vec<String>,
    pub modes: Vec<DlssMode>,
}

#[derive(Deserialize, Clone)]
pub struct Shaders {
    pub env_nvidia: Map<String, Value>,
    pub threads_laisses_libres: usize,
    pub threads_min: usize,
}

#[derive(Deserialize, Clone)]
pub struct Reglages {
    pub profil: String,
    pub groupes: Vec<Group>,
    pub pipelines_sautees: Skip,
    pub petite_ram: LowRam,
    pub materiel: Materiel,
    pub dlss: Dlss,
    pub shaders: Shaders,
}

impl Reglages {
    pub fn load(paths: &Paths) -> Result<Self, String> {
        let b = std::fs::read(&paths.reglages).map_err(|e| format!("{}: {e}", paths.reglages.display()))?;
        serde_json::from_slice(strip_bom(&b)).map_err(|e| format!("{}: {e}", paths.reglages.display()))
    }

    /// The pipelines victory.exe skips: the ray tracing file (trimmed, as Get-Content -Raw then Trim) then the extra hashes.
    pub fn skip_value(&self, root: &Path) -> String {
        let mut parts = Vec::new();
        if let Ok(text) = std::fs::read_to_string(root.join(&self.pipelines_sautees.fichier)) {
            parts.push(text.trim().to_string());
        }
        parts.extend(self.pipelines_sautees.en_plus.iter().cloned());
        parts.join(",")
    }

    pub fn dlss_dir_ok(&self, dir: &str) -> bool {
        !dir.is_empty() && Path::new(dir).join(&self.dlss.fichier_requis).is_file()
    }

    pub fn shader_jobs(&self) -> usize {
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        n.saturating_sub(self.shaders.threads_laisses_libres).max(self.shaders.threads_min)
    }
}

fn value_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// What to change in the inherited environment for victory.exe.
#[derive(Default, Clone)]
pub struct EnvPlan {
    pub set: Vec<(String, String)>,
    pub remove: Vec<String>,
    pub path_prepend: Option<PathBuf>,
}

impl EnvPlan {
    pub fn apply(&self, cmd: &mut std::process::Command) {
        if let Some(dir) = &self.path_prepend {
            let path = std::env::var("PATH").unwrap_or_default();
            cmd.env("PATH", format!("{};{path}", dir.display()));
        }
        for (k, v) in &self.set {
            cmd.env(k, v);
        }
        for k in &self.remove {
            cmd.env_remove(k);
        }
    }
}

/// The newest Windows SDK folder with dxil.dll (as reglages.ps1: Sort-Object FullName, last).
fn dxil_dir() -> Option<PathBuf> {
    let base = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut found: Vec<PathBuf> = std::fs::read_dir(base)
        .ok()?
        .flatten()
        .map(|e| e.path().join("x64"))
        .filter(|d| d.join("dxil.dll").is_file())
        .collect();
    found.sort_by_key(|d| d.join("dxil.dll").display().to_string().to_lowercase());
    found.pop()
}

/// The groups' variables, in the file's order.
pub fn base_env(r: &Reglages) -> Vec<(String, String)> {
    let mut set = Vec::new();
    for g in &r.groupes {
        for (k, v) in &g.env {
            put(&mut set, k, value_str(v));
        }
    }
    set
}

/// The hardware profile of this PC (see profile.rs).
pub fn profile(r: &Reglages, hw: &Hardware) -> Profile {
    r.materiel.profile(hw, &base_env(r), &r.petite_ram)
}

/// Set-GameEnv without the PATH and the files: VICTORY_CONFIG, the Series S settings adjusted by the hardware
/// profile (which holds the small-RAM rule), the skipped pipelines, DLSS (RTX cards only).
pub fn game_env_for(r: &Reglages, s: &Settings, hw: &Hardware, toml: &str, skip: &str) -> EnvPlan {
    let mut plan = EnvPlan::default();
    let mut set: Vec<(String, String)> = vec![("VICTORY_CONFIG".into(), toml.to_string())];
    let base = base_env(r);
    set.extend(base.iter().cloned());
    let prof = r.materiel.profile(hw, &base, &r.petite_ram);
    for (k, v) in &prof.env {
        put(&mut set, k, v.clone());
    }
    set.push((r.pipelines_sautees.variable.clone(), skip.to_string()));
    let d = &r.dlss;
    let mut remove = vec![d.variable_dossier.clone(), d.variable_mode.clone()];
    let (dir, mode) = (s.str("dlss_dir"), s.str("dlss_mode"));
    if prof.dlss && !mode.is_empty() && mode != "off" && r.dlss_dir_ok(&dir) {
        remove.clear();
        set.push((d.variable_dossier.clone(), dir));
        set.push((d.variable_mode.clone(), mode));
        set.retain(|(k, _)| !d.retire.contains(k));
        remove.extend(d.retire.iter().cloned());
    }
    plan.set = set;
    plan.remove = remove;
    plan
}

/// Set-GameEnv: game_env_for with this repository's files and the Windows SDK folder in PATH.
/// An inherited VICTORY_CACHE_DIR (or any other variable not named here) is left as it is.
pub fn game_env(paths: &Paths, r: &Reglages, s: &Settings, hw: &Hardware) -> EnvPlan {
    let mut plan = game_env_for(r, s, hw, &paths.toml.display().to_string(), &r.skip_value(&paths.root));
    if let Some(dir) = dxil_dir() {
        let path = std::env::var("PATH").unwrap_or_default();
        if !path.starts_with(&format!("{};", dir.display())) {
            plan.path_prepend = Some(dir);
        }
    }
    plan
}

/// VICTORY_HOST_WAVE and friends for « victory shaders » on NVIDIA (profile.shaders_nvidia).
pub fn shader_host_env(r: &Reglages) -> Vec<(String, String)> {
    r.shaders.env_nvidia.iter().map(|(k, v)| (k.clone(), value_str(v))).collect()
}
