//! lanceur.json (the player's choices, shared with the console launcher) and series-s/reglages.json (the
//! Series S settings, the one source also read by series-s/reglages.ps1). `game_env` applies them exactly as
//! Set-GameEnv in reglages.ps1 does.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::msg::Msg;
use crate::paths::Paths;
pub use crate::profile::LowRam;
use crate::profile::{Hardware, Materiel, Profile, put};

// ------------------------------------------------------------------ lanceur.json

/// lanceur.json as a JSON object: keys this launcher does not know are kept as they are.
pub struct Settings(pub Map<String, Value>);

/// Keys of lanceur.json, with their default when an older file lacks them (as Complete-Settings in reglages.ps1).
/// pipelines_when (this launcher only): Unix time of the last successful « victory warm --used ».
/// pipelines_liste: « nom:taille » of the shared list the last successful « warm --used » built (as the console).
/// pipelines_import (this launcher only): the list whose shader containers are in the cache (imported with
/// --shaders, or before a translation that kept them: pipelines_import_pending until it succeeds).
/// framegen, petite_vram: the frame generation and low video memory options (off by default).
/// clavier_lettres: keys read by their printed letter, not their place (« controles », off by default).
fn defaults() -> [(&'static str, Value); 15] {
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
        ("pipelines_liste", json!("")),
        ("pipelines_import", json!("")),
        ("pipelines_import_pending", json!("")),
        ("framegen", json!(false)),
        ("petite_vram", json!(false)),
        ("clavier_lettres", json!(false)),
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
    /// For the window's English texts (« group.<id>.titre »); the French ones are titre and note.
    #[serde(default)]
    pub id: String,
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

/// « jeu » in reglages.json: what a complete copy of the game holds.
#[derive(Deserialize, Clone)]
pub struct GameRule {
    pub exe: String,
    pub archives_requises: Vec<String>,
    pub archives_min: usize,
    pub taille_min_go: f64,
    pub shaders_min: i64,
    pub conseil: String,
}

/// A copy of the game, checked: its archives beside the exe.
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct GameCheck {
    pub path: String,
    pub complete: bool,
    pub rpf_count: usize,
    pub size_gb: f64,
    /// What is missing, with the advice (None when complete): « game.incomplete », translated by the window.
    pub problem: Option<Msg>,
}

impl GameCheck {
    /// The problem as a command's error (empty when complete).
    pub fn problem_err(&self) -> String {
        self.problem.clone().map(String::from).unwrap_or_default()
    }

    /// The problem for developers and tests (`key(params)`).
    #[cfg(test)]
    pub fn problem_text(&self) -> String {
        self.problem.as_ref().map(|m| m.to_string()).unwrap_or_default()
    }
}

impl GameRule {
    /// Checks the folder of `exe` (only names and sizes are read, never the files themselves).
    pub fn check(&self, exe: &str) -> GameCheck {
        let mut c = GameCheck { path: exe.to_string(), ..Default::default() };
        let exe_path = Path::new(exe);
        let dir = exe_path.parent().unwrap_or(Path::new(""));
        let mut rpfs: Vec<String> = Vec::new();
        let mut size = 0u64;
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_lowercase();
                if name.ends_with(".rpf") {
                    size += e.metadata().map(|m| m.len()).unwrap_or(0);
                    rpfs.push(name);
                }
            }
        }
        c.rpf_count = rpfs.len();
        c.size_gb = size as f64 / (1u64 << 30) as f64;
        let mut missing: Vec<String> = Vec::new();
        if !exe_path.is_file() {
            missing.push(self.exe.clone());
        }
        missing.extend(self.archives_requises.iter().filter(|a| !rpfs.contains(&a.to_lowercase())).cloned());
        let few = c.rpf_count < self.archives_min;
        let small = c.size_gb < self.taille_min_go;
        c.complete = missing.is_empty() && !few && !small;
        if !c.complete {
            let mut what = Vec::new();
            if !missing.is_empty() {
                what.push(Msg::new("game.missing").with("files", missing.join(", ")));
            }
            if few {
                what.push(Msg::new("game.fewArchives").with("count", c.rpf_count).with("min", self.archives_min));
            }
            if small {
                what.push(Msg::new("game.smallSize").with("size", format!("{:.0}", c.size_gb)).with("min", format!("{:.0}", self.taille_min_go)));
            }
            c.problem = Some(
                Msg::new("game.incomplete")
                    .with("dir", dir.display().to_string())
                    .with("what", what)
                    .with("advice", Msg::data("jeu.conseil", &self.conseil)),
            );
        }
        c
    }
}

/// The number of shaders a « victory shaders » run put in the cache, from its log: its last line
/// « N shaders in the cache, M do not translate, S s ».
pub fn shaders_count_from_log(log: &Path) -> Option<i64> {
    let text = std::fs::read_to_string(log).ok()?;
    text.lines().rev().find_map(|l| {
        let (n, rest) = l.trim().split_once(' ')?;
        rest.starts_with("shaders in the cache, ").then_some(())?;
        rest.contains("do not translate").then_some(())?;
        n.parse().ok()
    })
}

/// « pipelines_partagees » in reglages.json: the shared pipeline list (no shader code).
#[derive(Deserialize, Clone)]
pub struct SharedList {
    pub fichier: String,
}

/// The shared list as the window shows it.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SharedListView {
    pub file: String,
    /// « series-s-rtx.txt:454981 » (as Get-SharedPipelineMark in reglages.ps1).
    pub mark: String,
    /// Pipelines it holds, from its « # N pipelines » line.
    pub count: Option<u64>,
}

impl SharedList {
    pub fn view(&self, root: &Path) -> Option<SharedListView> {
        let path = root.join(&self.fichier);
        let len = std::fs::metadata(&path).ok().filter(|m| m.is_file())?.len();
        let name = path.file_name()?.to_string_lossy().to_string();
        let count = std::fs::read_to_string(&path).ok().and_then(|t| {
            t.lines().take(20).find_map(|l| l.strip_prefix("# ")?.split_once(" pipelines")?.0.trim().parse().ok())
        });
        Some(SharedListView { file: path.display().to_string(), mark: format!("{name}:{len}"), count })
    }
}

/// « petite_vram » in reglages.json: the low video memory mode (the player's choice), advised under
/// conseille_sous_go GB of card memory.
#[derive(Deserialize, Clone)]
pub struct LowVram {
    pub conseille_sous_go: f64,
    pub env: Map<String, Value>,
}

/// « framegen.fsr3 »: dlssg-to-fsr3's files in the package's fg folder.
#[derive(Deserialize, Clone)]
pub struct Fsr3 {
    pub dossier_paquet: String,
    pub fichiers: Vec<String>,
}

/// « framegen » in reglages.json: frame generation (the player's choice), on the cards that have it.
#[derive(Deserialize, Clone)]
pub struct Framegen {
    pub env: Map<String, Value>,
    pub plafonds: Map<String, Value>,
    pub variable_dossier: String,
    pub sans_dlss: Map<String, Value>,
    pub dossier: String,
    pub cartes_fsr3: Vec<String>,
    pub cartes_nvidia: Vec<String>,
    pub fichiers_requis: Vec<String>,
    pub fichiers_dlss: Vec<String>,
    pub fsr3: Fsr3,
}

impl Framegen {
    /// What frame generation needs (fichiers_requis) and `dir` lacks.
    pub fn missing(&self, dir: &Path) -> Vec<String> {
        self.fichiers_requis.iter().filter(|f| !dir.join(f).is_file()).cloned().collect()
    }

    pub fn files_ok(&self, dir: &Path) -> bool {
        self.missing(dir).is_empty()
    }

    /// "fsr3", "nvidia" or "" for this card (profile::framegen_kind).
    pub fn kind(&self, gpu_name: &str) -> &'static str {
        crate::profile::framegen_kind(gpu_name, &self.cartes_fsr3, &self.cartes_nvidia)
    }
}

/// « controles » in reglages.json: the keyboard and mouse layout (documented there); the launcher sets the variable
/// that reads keys by their printed letter when the player asks (« clavier_lettres »).
#[derive(Deserialize, Clone)]
pub struct Controls {
    pub variable_clavier: String,
    pub valeur_lettres: String,
}

#[derive(Deserialize, Clone)]
pub struct Reglages {
    pub profil: String,
    pub jeu: GameRule,
    pub groupes: Vec<Group>,
    pub pipelines_sautees: Skip,
    pub petite_ram: LowRam,
    pub pipelines_partagees: SharedList,
    pub materiel: Materiel,
    pub dlss: Dlss,
    pub petite_vram: LowVram,
    pub framegen: Framegen,
    pub controles: Controls,
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

/// The hardware profile of this PC (see profile.rs), with what the options of reglages.json say of this card.
pub fn profile(r: &Reglages, hw: &Hardware) -> Profile {
    let mut p = r.materiel.profile(hw, &base_env(r), &r.petite_ram);
    p.framegen = r.framegen.kind(&hw.gpu_name).to_string();
    p.low_vram_advised = hw.vram_bytes > 0 && (hw.vram_bytes as f64 / (1u64 << 30) as f64) < r.petite_vram.conseille_sous_go;
    p
}

/// The frame generation folder, in the launcher's own (%LOCALAPPDATA%\Sagouindupm\framegen).
pub fn framegen_dir(paths: &Paths, r: &Reglages) -> PathBuf {
    paths.home.join(&r.framegen.dossier)
}

/// Set-GameEnv without the PATH and the files: VICTORY_CONFIG, the Series S settings adjusted by the hardware
/// profile (which holds the small-RAM rule), the low video memory mode and frame generation if chosen (`fg_dir`:
/// the frame generation folder), the skipped pipelines, DLSS (RTX cards only). With both options off, the same as
/// before them.
pub fn game_env_for(r: &Reglages, s: &Settings, hw: &Hardware, toml: &str, skip: &str, fg_dir: &Path) -> EnvPlan {
    let mut plan = EnvPlan::default();
    let mut set: Vec<(String, String)> = vec![("VICTORY_CONFIG".into(), toml.to_string())];
    let base = base_env(r);
    set.extend(base.iter().cloned());
    let prof = r.materiel.profile(hw, &base, &r.petite_ram);
    for (k, v) in &prof.env {
        put(&mut set, k, v.clone());
    }
    // Low video memory mode: see « petite_vram » in reglages.json.
    if s.bool("petite_vram") {
        for (k, v) in &r.petite_vram.env {
            put(&mut set, k, value_str(v));
        }
    }
    // Keys by their printed letter: the player's choice (« controles »).
    if s.bool("clavier_lettres") {
        put(&mut set, &r.controles.variable_clavier, r.controles.valeur_lettres.clone());
    }
    // Frame generation: the player's choice, a card that has it, its files there (« framegen »).
    let fg = &r.framegen;
    let fg_on = s.bool("framegen") && !fg.kind(&hw.gpu_name).is_empty() && fg.files_ok(fg_dir);
    let fg_path = fg_dir.display().to_string();
    if fg_on {
        for (k, v) in &fg.env {
            put(&mut set, k, value_str(v));
        }
        for (k, cap) in &fg.plafonds {
            let cap = cap.as_f64().unwrap_or(f64::MAX);
            let now = set.iter().find(|(sk, _)| sk == k).and_then(|(_, v)| v.parse::<f64>().ok());
            if now.is_none_or(|n| n > cap) {
                put(&mut set, k, format!("{cap}"));
            }
        }
        put(&mut set, &fg.variable_dossier, fg_path.clone());
    }
    set.push((r.pipelines_sautees.variable.clone(), skip.to_string()));
    let d = &r.dlss;
    let mut remove = vec![d.variable_dossier.clone(), d.variable_mode.clone()];
    let mode = s.str("dlss_mode");
    // With frame generation on, Streamline is loaded once from VICTORY_DLSS_DIR, which must then hold both: the
    // frame generation folder, if it has DLSS's files.
    let dir = if fg_on {
        if fg.fichiers_dlss.iter().all(|f| fg_dir.join(f).is_file()) { fg_path } else { String::new() }
    } else {
        Some(s.str("dlss_dir")).filter(|d| r.dlss_dir_ok(d)).unwrap_or_default()
    };
    if prof.dlss && !mode.is_empty() && mode != "off" && !dir.is_empty() {
        remove.clear();
        set.push((d.variable_dossier.clone(), dir));
        set.push((d.variable_mode.clone(), mode));
        set.retain(|(k, _)| !d.retire.contains(k));
        remove.extend(d.retire.iter().cloned());
    } else if fg_on {
        // The title's own upscaler: DLSS super resolution off, the folder serves frame generation only.
        for (k, v) in &fg.sans_dlss {
            put(&mut set, k, value_str(v));
        }
        remove = vec![d.variable_dossier.clone()];
    }
    plan.set = set;
    plan.remove = remove;
    plan
}

/// Set-GameEnv: game_env_for with this repository's files and the Windows SDK folder in PATH.
/// An inherited VICTORY_CACHE_DIR (or any other variable not named here) is left as it is.
pub fn game_env(paths: &Paths, r: &Reglages, s: &Settings, hw: &Hardware) -> EnvPlan {
    game_env_in(paths, r, s, hw, &framegen_dir(paths, r))
}

/// game_env with another frame generation folder (--print-env --framegen-dir).
pub fn game_env_in(paths: &Paths, r: &Reglages, s: &Settings, hw: &Hardware, fg_dir: &Path) -> EnvPlan {
    let mut plan = game_env_for(r, s, hw, &paths.toml.display().to_string(), &r.skip_value(&paths.root), fg_dir);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of empty files with the names of a copy of the game (synthetic: no game data).
    fn copy_with(names: &[&str]) -> tempdir::Dir {
        let d = tempdir::Dir::new();
        for n in names {
            std::fs::write(d.0.join(n), b"").unwrap();
        }
        d
    }

    mod tempdir {
        pub struct Dir(pub std::path::PathBuf);
        impl Dir {
            pub fn new() -> Self {
                use std::sync::atomic::{AtomicU32, Ordering};
                static N: AtomicU32 = AtomicU32::new(0);
                let p = std::env::temp_dir().join(format!("victory-s-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
                std::fs::create_dir_all(&p).unwrap();
                Dir(p)
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    fn rule() -> GameRule {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../series-s/reglages.json");
        let b = std::fs::read(path).unwrap();
        let r: Reglages = serde_json::from_slice(b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&b)).unwrap();
        // Empty files here: the size rule is left out of this test.
        GameRule { taille_min_go: 0.0, ..r.jeu }
    }

    #[test]
    fn complete_copy_of_the_game() {
        let names = [
            "game_scarlett_master.exe", "anim.rpf", "appdata0.rpf", "audio.rpf", "common.rpf", "data.rpf", "levels.rpf",
            "movies.rpf", "navmeshes.rpf", "packs.rpf", "script.rpf", "shaders.rpf", "textures.rpf",
        ];
        let d = copy_with(&names);
        let c = rule().check(&d.0.join("game_scarlett_master.exe").display().to_string());
        assert!(c.complete, "{}", c.problem_text());
        assert!(c.problem.is_none());
        assert_eq!(c.rpf_count, 12);
    }

    #[test]
    fn exe_alone_is_incomplete() {
        let d = copy_with(&["game_scarlett_master.exe", "common.rpf"]);
        let c = rule().check(&d.0.join("game_scarlett_master.exe").display().to_string());
        assert!(!c.complete);
        // The message for the window (« game.incomplete », translated there), with reglages.json's advice.
        let p = c.problem_text();
        assert!(p.starts_with("game.incomplete("), "{p}");
        assert!(p.contains("shaders.rpf") && p.contains("levels.rpf"), "{p}");
        assert!(p.contains("game.fewArchives(count=1, min=8)"), "{p}");
        assert!(p.contains("pas en vrac dans Téléchargements"), "{p}");
        assert!(c.problem_err().starts_with(r#"{"key":"game.incomplete","#), "{}", c.problem_err());
        // And the size rule of reglages.json: empty archives are far from a complete copy.
        let mut r = rule();
        r.taille_min_go = 50.0;
        let names = ["game_scarlett_master.exe", "a.rpf", "b.rpf", "c.rpf", "d.rpf", "e.rpf", "f.rpf", "levels.rpf", "shaders.rpf"];
        let d = copy_with(&names);
        let c = r.check(&d.0.join("game_scarlett_master.exe").display().to_string());
        assert!(!c.complete && c.problem_text().contains("game.smallSize("), "{}", c.problem_text());
    }

    #[test]
    fn shared_list_of_the_repository() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let b = std::fs::read(root.join("series-s/reglages.json")).unwrap();
        let r: Reglages = serde_json::from_slice(b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&b)).unwrap();
        let v = r.pipelines_partagees.view(&root).expect("series-s/pipelines/series-s-rtx.txt");
        assert_eq!(v.count, Some(3535));
        assert!(v.mark.starts_with("series-s-rtx.txt:"), "{}", v.mark);
    }

    #[test]
    fn shaders_count_in_the_log() {
        let d = tempdir::Dir::new();
        let log = d.0.join("shaders_traduction.txt");
        std::fs::write(&log, "1000 shaders in the cache, 12 s\n25312 shaders in the cache, 41 do not translate, 1210 s\n      41  unsupported  (e.g. 0011)\n").unwrap();
        assert_eq!(shaders_count_from_log(&log), Some(25312));
        std::fs::write(&log, "1000 shaders in the cache, 12 s\n").unwrap();
        assert_eq!(shaders_count_from_log(&log), None);
    }
}
