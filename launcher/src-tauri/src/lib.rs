//! Sagouindupm | VIctory: the graphical launcher for novices (Tauri). The window (launcher/src) shows the state
//! and asks; everything that touches the system is here, in Rust: reading series-s/reglages.json and
//! lanceur.json, the checks, the long tasks as child processes, launching the game exactly like the console
//! launcher (series-s/Sagouindupm.ps1).

mod jobs;
mod paths;
mod profile;
mod settings;
mod sys;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use jobs::{Jobs, Spec};
use paths::Paths;
use profile::{Hardware, Profile};
use settings::{DlssMode, Reglages, Settings, game_env, shader_host_env, write_victory_toml};

struct App {
    paths: Paths,
    jobs: Jobs,
    facts: Mutex<Option<sys::Facts>>,
    last_game_log: Mutex<Option<PathBuf>>,
    /// `--preview <dir>`: the window shows each page in turn and saves a screenshot of it in <dir>, then closes.
    preview: Option<PathBuf>,
}

/// This launcher's version (launcher/src-tauri/Cargo.toml, the same as the package's).
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What the profile is computed from: the checks once done (graphics card), else RAM and threads only.
fn hardware_of(f: Option<&sys::Facts>) -> Hardware {
    Hardware {
        ram_bytes: sys::total_ram(),
        vram_bytes: f.map(|f| f.vram.max(0.0) as u64).unwrap_or(0),
        threads: f.map(|f| f.threads).filter(|&t| t > 0).unwrap_or_else(sys::threads),
        gpu_name: f.map(|f| f.gpu_name.clone()).unwrap_or_default(),
        avx2: f.map(|f| f.avx2),
    }
}

/// The hardware for a task that starts victory.exe: the checks are done first if they were not.
fn hardware(app: &App) -> Hardware {
    let mut f = app.facts.lock().unwrap();
    if f.is_none() {
        *f = Some(sys::facts());
    }
    hardware_of(f.as_ref())
}

type Res<T> = Result<T, String>;

// ------------------------------------------------------------------ snapshot: everything the window shows

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    game_exe: String,
    shaders_dir: String,
    shaders_ready: bool,
    parties: i64,
    dlss_dir: String,
    dlss_mode: String,
    pipelines_parties: i64,
    pipelines_when: Option<i64>,
    nv_cache_ok: bool,
    ancien_plan: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GroupView {
    titre: String,
    note: String,
    env: Vec<(String, String)>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileView {
    #[serde(flatten)]
    profile: Profile,
    /// The graphics card is not checked yet: the profile will change once it is.
    pending: bool,
    /// What the player copies into a feedback report.
    text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    root: String,
    exe_path: String,
    /// The downloaded package: victory.exe is included (engine\victory.exe).
    packaged: bool,
    version: String,
    /// The hardware profile of this PC (series-s/reglages.json, « materiel »).
    hardware: Option<ProfileView>,
    /// victory.exe's date (Unix ms), if compiled.
    exe_built: Option<u64>,
    reglages_error: Option<String>,
    settings: SettingsView,
    game_ok: bool,
    dlss_ok: bool,
    dlss_missing: Vec<String>,
    ram_bytes: u64,
    power_line: String,
    last_log: Option<String>,
    logs_dir: String,
    profil: String,
    groups: Vec<GroupView>,
    skip_count: usize,
    skip_file: String,
    low_ram_note: String,
    low_ram_below_gb: f64,
    low_ram_env: Vec<(String, String)>,
    dlss_note: String,
    dlss_modes: Vec<DlssMode>,
    /// The variables victory.exe gets now (the skip list summarised).
    applied: Vec<(String, String)>,
    running: Option<String>,
    warm_done: bool,
    cache_dir: String,
    cache_inherited: bool,
}

fn mtime_ms(p: &Path) -> Option<u64> {
    let t = std::fs::metadata(p).ok()?.modified().ok()?;
    t.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_millis() as u64)
}

fn last_game_log(app: &App) -> Option<PathBuf> {
    if let Some(p) = app.last_game_log.lock().unwrap().clone().filter(|p| p.is_file()) {
        return Some(p);
    }
    std::fs::read_dir(&app.paths.logs)
        .ok()?
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.starts_with("partie_") && n.ends_with(".log")
        })
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

fn kv(m: &serde_json::Map<String, serde_json::Value>) -> Vec<(String, String)> {
    m.iter().map(|(k, v)| (k.clone(), v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string()))).collect()
}

#[tauri::command]
fn snapshot(app: State<'_, App>) -> Res<Snapshot> {
    let p = &app.paths;
    let s = Settings::load(p);
    let reglages = Reglages::load(p);
    let facts = app.facts.lock().unwrap().clone();
    let hw = hardware_of(facts.as_ref());
    let ram = hw.ram_bytes;
    let dlss_dir = s.str("dlss_dir");
    let mut snap = Snapshot {
        root: p.root.display().to_string(),
        exe_path: p.exe.display().to_string(),
        packaged: p.packaged,
        version: VERSION.to_string(),
        hardware: None,
        exe_built: p.exe.is_file().then(|| mtime_ms(&p.exe)).flatten(),
        reglages_error: reglages.as_ref().err().cloned(),
        settings: SettingsView {
            game_exe: s.str("game_exe"),
            shaders_dir: s.str("shaders_dir"),
            shaders_ready: s.bool("shaders_ready"),
            parties: s.int("parties"),
            dlss_dir: dlss_dir.clone(),
            dlss_mode: s.str("dlss_mode"),
            pipelines_parties: s.int("pipelines_parties"),
            pipelines_when: s.0.get("pipelines_when").and_then(|v| v.as_i64()),
            nv_cache_ok: s.bool("nv_cache_ok"),
            ancien_plan: s.str("ancien_plan"),
        },
        game_ok: s.game_ok(),
        dlss_ok: false,
        dlss_missing: Vec::new(),
        ram_bytes: ram,
        power_line: sys::power_line().to_string(),
        last_log: last_game_log(&app).map(|p| p.display().to_string()),
        logs_dir: p.logs.display().to_string(),
        profil: String::new(),
        groups: Vec::new(),
        skip_count: 0,
        skip_file: String::new(),
        low_ram_note: String::new(),
        low_ram_below_gb: 24.0,
        low_ram_env: Vec::new(),
        dlss_note: String::new(),
        dlss_modes: Vec::new(),
        applied: Vec::new(),
        running: app.jobs.running(),
        warm_done: sys::warm_done(),
        cache_dir: paths::victory_dir("VICTORY_CACHE_DIR", "cache").display().to_string(),
        cache_inherited: std::env::var_os("VICTORY_CACHE_DIR").is_some_and(|v| !v.is_empty()),
    };
    if let Ok(r) = &reglages {
        snap.dlss_ok = r.dlss_dir_ok(&dlss_dir);
        if snap.dlss_ok {
            snap.dlss_missing = ["sl.common.dll", "sl.dlss.dll", "nvngx_dlss.dll"]
                .iter()
                .filter(|f| !Path::new(&dlss_dir).join(f).is_file())
                .map(|f| f.to_string())
                .collect();
        }
        snap.profil = r.profil.clone();
        snap.groups = r.groupes.iter().map(|g| GroupView { titre: g.titre.clone(), note: g.note.clone(), env: kv(&g.env) }).collect();
        let skip = r.skip_value(&p.root);
        snap.skip_count = skip.split(',').filter(|h| !h.trim().is_empty()).count();
        snap.skip_file = r.pipelines_sautees.fichier.clone();
        snap.low_ram_note = r.petite_ram.note.clone();
        snap.low_ram_below_gb = r.petite_ram.moins_de_go;
        snap.low_ram_env = kv(&r.petite_ram.env);
        snap.dlss_note = r.dlss.note.clone();
        snap.dlss_modes = r.dlss.modes.clone();
        let prof = settings::profile(r, &hw);
        let text = profile_text(&prof, &s, r, snap.dlss_ok);
        snap.hardware = Some(ProfileView { profile: prof, pending: facts.is_none(), text });
        let plan = game_env(p, r, &s, &hw);
        snap.applied = plan
            .set
            .into_iter()
            .map(|(k, v)| if k == r.pipelines_sautees.variable { (k, format!("{} programmes", snap.skip_count)) } else { (k, v) })
            .collect();
    }
    Ok(snap)
}

// ------------------------------------------------------------------ checks

#[tauri::command]
async fn facts(app: AppHandle) -> Res<sys::Facts> {
    let f = tauri::async_runtime::spawn_blocking(sys::facts).await.map_err(|e| e.to_string())?;
    *app.state::<App>().facts.lock().unwrap() = Some(f.clone());
    Ok(f)
}

// ------------------------------------------------------------------ the game's folder

fn set_game_path(app: &App, path: &str) -> Res<()> {
    let p = Path::new(path);
    if !p.is_file() || !path.to_lowercase().ends_with(".exe") {
        return Err(format!("Fichier introuvable : {path}"));
    }
    let mut s = Settings::load(&app.paths);
    s.set("game_exe", path);
    write_victory_toml(&app.paths, path)?;
    s.save(&app.paths)
}

#[tauri::command]
async fn find_games() -> Res<Vec<String>> {
    tauri::async_runtime::spawn_blocking(sys::find_games).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn set_game(app: State<'_, App>, path: String) -> Res<()> {
    set_game_path(&app, &path)
}

#[tauri::command]
async fn pick_game(handle: AppHandle) -> Res<Option<String>> {
    let h = handle.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let mut d = h.dialog().file().set_title("Où est game_scarlett_master.exe (votre copie du jeu) ?").add_filter("Jeu (game_scarlett_master.exe)", &["exe"]);
        if let Some(w) = h.get_webview_window("main") {
            d = d.set_parent(&w);
        }
        d.blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(path) = picked.and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let path = path.display().to_string();
    set_game_path(&handle.state::<App>(), &path)?;
    Ok(Some(path))
}

// ------------------------------------------------------------------ DLSS

#[tauri::command]
async fn pick_dlss(handle: AppHandle) -> Res<Option<String>> {
    let h = handle.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let mut d = h.dialog().file().set_title("Dossier qui contient sl.interposer.dll (Streamline, bin\\x64)");
        if let Some(w) = h.get_webview_window("main") {
            d = d.set_parent(&w);
        }
        d.blocking_pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(dir) = picked.and_then(|f| f.into_path().ok()) else { return Ok(None) };
    let app = handle.state::<App>();
    let r = Reglages::load(&app.paths)?;
    let dir = dir.display().to_string();
    if !r.dlss_dir_ok(&dir) {
        return Err(format!("{} introuvable dans ce dossier.", r.dlss.fichier_requis));
    }
    let mut s = Settings::load(&app.paths);
    s.set("dlss_dir", dir.clone());
    s.save(&app.paths)?;
    Ok(Some(dir))
}

/// "off" (the Series S settings: 540p + the game's upscaler) or a DLSS mode of reglages.json.
#[tauri::command]
fn set_mode(app: State<'_, App>, mode: String) -> Res<()> {
    let r = Reglages::load(&app.paths)?;
    if mode != "off" && !r.dlss.modes.iter().any(|m| m.id == mode) {
        return Err(format!("Mode inconnu : {mode}"));
    }
    let mut s = Settings::load(&app.paths);
    s.set("dlss_mode", mode);
    s.save(&app.paths)
}

// ------------------------------------------------------------------ long tasks

fn shaders_source(s: &Settings) -> String {
    let dir = s.str("shaders_dir");
    if !dir.is_empty() && Path::new(&dir).exists() {
        return dir;
    }
    // Without an extracted folder: the game's shaders.rpf, else the whole game folder (victory shaders looks for
    // shader containers and SGD5 records in every file).
    let game_dir = Path::new(&s.str("game_exe")).parent().map(Path::to_path_buf).unwrap_or_default();
    let rpf = game_dir.join("shaders.rpf");
    if rpf.is_file() { rpf.display().to_string() } else { game_dir.display().to_string() }
}

fn build_spec(app: &App, kind: &str) -> Res<Spec> {
    let p = &app.paths;
    let s = Settings::load(p);
    let log = |name: &str| p.logs.join(name);
    let winget = |args: &[&str], name: &str| -> Res<Spec> {
        let w = sys::find_winget().ok_or("winget est absent de ce Windows : installez « App Installer » depuis le Microsoft Store.")?;
        let mut cmd = Command::new(w);
        cmd.args(["install", "-e", "--accept-package-agreements", "--accept-source-agreements"]).args(args);
        Ok(Spec { kind: kind.into(), cmd, log: log(name), game: false, capture_dir: None })
    };
    let need_exe = || if p.exe.is_file() { Ok(()) } else { Err("VIctory n'est pas encore compilé (étape 1).".to_string()) };
    let need_game = || if s.game_ok() { Ok(()) } else { Err("Indiquez d'abord où est le jeu (étape 2).".to_string()) };
    match kind {
        "rust" => winget(&["--id", "Rustlang.Rustup"], "installation_rust.txt"),
        "vs" => winget(
            &["--id", "Microsoft.VisualStudio.2022.BuildTools", "--override", "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"],
            "installation_outils_cpp.txt",
        ),
        "build" if p.packaged => Err("Cette version téléchargée contient déjà VIctory (engine\\victory.exe) : rien à compiler.".into()),
        "build" => {
            let cargo = sys::find_cargo().ok_or("Rust n'est pas installé (étape 1).")?;
            let mut cmd = Command::new(&cargo);
            cmd.args(["build", "--release", "--locked", "-p", "victory"]).current_dir(&p.root);
            if let Some(dir) = cargo.parent() {
                cmd.env("PATH", format!("{};{}", dir.display(), std::env::var("PATH").unwrap_or_default()));
            }
            // victory.exe must land in <dépôt>\target\release, where both launchers look for it.
            cmd.env_remove("CARGO_TARGET_DIR");
            Ok(Spec { kind: kind.into(), cmd, log: log("compilation.txt"), game: false, capture_dir: None })
        }
        "shaders" => {
            need_exe()?;
            need_game()?;
            let r = Reglages::load(p)?;
            let mut cmd = Command::new(&p.exe);
            cmd.arg("shaders").arg(shaders_source(&s)).arg("--jobs").arg(r.shader_jobs().to_string());
            let hw = hardware(app);
            game_env(p, &r, &s, &hw).apply(&mut cmd);
            // NVIDIA only (waves of 32...): an AMD card keeps the generic Series S translation.
            if settings::profile(&r, &hw).shaders_nvidia {
                for (k, v) in shader_host_env(&r) {
                    cmd.env(k, v);
                }
            }
            Ok(Spec { kind: kind.into(), cmd, log: log("shaders_traduction.txt"), game: false, capture_dir: None })
        }
        "warm" => {
            need_exe()?;
            let r = Reglages::load(p)?;
            let mut cmd = Command::new(&p.exe);
            cmd.args(["warm", "--used"]);
            game_env(p, &r, &s, &hardware(app)).apply(&mut cmd);
            Ok(Spec { kind: kind.into(), cmd, log: log("pipelines_warm.txt"), game: false, capture_dir: None })
        }
        "play" => {
            need_exe()?;
            need_game()?;
            let r = Reglages::load(p)?;
            let (y, mo, d, h, mi, _) = sys::local_time();
            let log = log(&format!("partie_{y:04}-{mo:02}-{d:02}_{h:02}{mi:02}.log"));
            // As Sagouindupm.ps1: cmd /c "victory.exe" run "<jeu>" --console series-s --execute > journal 2>&1
            use std::os::windows::process::CommandExt;
            let mut cmd = Command::new("cmd.exe");
            cmd.raw_arg(format!(
                "/c \"\"{}\" run \"{}\" --console series-s --execute > \"{}\" 2>&1\"",
                p.exe.display(),
                s.str("game_exe"),
                log.display()
            ))
            .current_dir(p.root.join("series-s"));
            game_env(p, &r, &s, &hardware(app)).apply(&mut cmd);
            *app.last_game_log.lock().unwrap() = Some(log.clone());
            Ok(Spec { kind: kind.into(), cmd, log, game: true, capture_dir: Some(p.captures.clone()) })
        }
        _ => Err(format!("Tâche inconnue : {kind}")),
    }
}

/// Runs a task to its end; its lines arrive as "job-line" events. Returns the exit code.
#[tauri::command]
async fn run_job(handle: AppHandle, kind: String) -> Res<i32> {
    let h = handle.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let app = h.state::<App>();
        let spec = build_spec(&app, &kind)?;
        let code = app.jobs.run(&h, spec)?;
        let p = &app.paths;
        let mut s = Settings::load(p);
        match kind.as_str() {
            "shaders" if code == 0 => s.set("shaders_ready", true),
            "warm" if code == 0 => {
                let n = s.int("parties");
                s.set("pipelines_parties", n);
                s.set("pipelines_when", sys::unix_now());
            }
            "play" => {
                let n = s.int("parties") + 1;
                s.set("parties", n);
            }
            _ => return Ok(code),
        }
        s.save(p)?;
        Ok(code)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn stop_job(app: State<'_, App>) {
    app.jobs.stop();
}

// ------------------------------------------------------------------ small actions

/// "high": « Performances élevées » (the plan before is kept in ancien_plan); "revert": back to it.
#[tauri::command]
async fn power(handle: AppHandle, action: String) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let app = handle.state::<App>();
        let mut s = Settings::load(&app.paths);
        if action == "revert" {
            let old = s.str("ancien_plan");
            if old.is_empty() || !sys::set_scheme(&old) {
                return Err("Impossible de revenir à l'ancien mode d'alimentation.".into());
            }
            s.set("ancien_plan", "");
            s.save(&app.paths)?;
            return Ok("Ancien mode d'alimentation rétabli.".into());
        }
        if sys::power_line() == "offline" {
            return Err("Le PC est sur batterie : branchez le chargeur.".into());
        }
        let before = sys::active_scheme();
        if sys::set_scheme("SCHEME_MIN") {
            if !before.is_empty() && s.str("ancien_plan").is_empty() {
                s.set("ancien_plan", before);
                s.save(&app.paths)?;
            }
            return Ok("Mode « Performances élevées » activé.".into());
        }
        // Recent laptops (modern standby) only have Windows' slider.
        let _ = sys::shell_open("ms-settings:powersleep");
        Err("Ce PC n'a pas le mode « Performances élevées » : dans Paramètres, choisissez « Mode d'alimentation : Performances optimales ».".into())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn nvidia_panel() -> bool {
    tauri::async_runtime::spawn_blocking(sys::open_nvidia_panel).await.unwrap_or(false)
}

#[tauri::command]
fn confirm_nv_cache(app: State<'_, App>, ok: bool) -> Res<()> {
    let mut s = Settings::load(&app.paths);
    s.set("nv_cache_ok", ok);
    s.save(&app.paths)
}

#[tauri::command]
fn open(app: State<'_, App>, target: String) -> Res<()> {
    let p = &app.paths;
    let t = match target.as_str() {
        "logs" => p.logs.display().to_string(),
        "last_log" => {
            let log = last_game_log(&app).ok_or("Aucune partie lancée pour le moment.")?;
            return Command::new("notepad.exe").arg(log).spawn().map(|_| ()).map_err(|e| e.to_string());
        }
        "game_dir" => Path::new(&Settings::load(p).str("game_exe")).parent().map(|d| d.display().to_string()).unwrap_or_default(),
        "perf_options" => std::env::var("SystemRoot").map(|r| format!(r"{r}\System32\SystemPropertiesPerformance.exe")).unwrap_or_default(),
        "power_settings" => "ms-settings:powersleep".into(),
        "streamline" => "https://github.com/NVIDIA-RTX/Streamline/releases".into(),
        "rustup" => "https://rustup.rs".into(),
        "build_tools" => "https://visualstudio.microsoft.com/visual-cpp-build-tools/".into(),
        "repo" => p.root.display().to_string(),
        "captures" => {
            let _ = std::fs::create_dir_all(&p.captures);
            p.captures.display().to_string()
        }
        "docs" => "https://github.com/sagouindupm/Mandy#readme".into(),
        "license" => "https://github.com/sagouindupm/Mandy/blob/main/LICENSE".into(),
        "issues" => "https://github.com/sagouindupm/Mandy/issues".into(),
        "releases" => "https://github.com/sagouindupm/Mandy/releases".into(),
        "feedback" => return sys::open_url(&feedback_url(&app)),
        _ => return Err(format!("Cible inconnue : {target}")),
    };
    if t.is_empty() {
        return Err("Rien à ouvrir.".into());
    }
    sys::shell_open(&t)
}

/// The player's screenshots, for the window's background (read through the asset protocol, never bundled).
#[tauri::command]
fn captures(app: State<'_, App>) -> Vec<String> {
    sys::list_captures(&app.paths.captures).into_iter().map(|p| p.display().to_string()).collect()
}

/// A screenshot of the running game now.
#[tauri::command]
async fn capture_now(handle: AppHandle) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let app = handle.state::<App>();
        sys::capture_game(&app.paths.captures).map(|p| p.display().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Copies victory.exe's saves (VICTORY_DATA_DIR or %LOCALAPPDATA%\victory\data) to
/// %LOCALAPPDATA%\Sagouindupm\sauvegardes\<date>.
#[tauri::command]
async fn backup_saves(handle: AppHandle) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let app = handle.state::<App>();
        let data = paths::victory_dir("VICTORY_DATA_DIR", "data");
        if !data.is_dir() {
            return Err("Aucune sauvegarde trouvée pour le moment.".to_string());
        }
        let (y, mo, d, h, mi, s) = sys::local_time();
        let dst = app.paths.home.join("sauvegardes").join(format!("{y:04}-{mo:02}-{d:02}_{h:02}{mi:02}{s:02}"));
        sys::copy_dir(&data, &dst).map_err(|e| e.to_string())?;
        Ok(dst.display().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// ------------------------------------------------------------------ feedback

/// The hardware profile as the player copies it into a feedback report (Réglages, « Copier »).
fn profile_text(p: &Profile, s: &Settings, r: &Reglages, dlss_ok: bool) -> String {
    let mode = s.str("dlss_mode");
    let mode = r
        .dlss
        .modes
        .iter()
        .find(|m| m.id == mode && dlss_ok && p.dlss)
        .map(|m| m.nom.clone())
        .unwrap_or_else(|| "Recommandé (540p, upscaler du jeu)".into());
    let mut lines = vec![format!("Profil détecté : Series S · {}", p.label), format!("Mode : {mode}"), format!("Crans : {}", p.tiers.join(", "))];
    lines.extend(p.reasons.iter().map(|l| format!("- {l}")));
    lines.extend(p.warnings.iter().map(|l| format!("! {l}")));
    if !p.env.is_empty() {
        lines.push(format!("Changé : {}", p.env.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")));
    }
    lines.join("\n")
}

/// Percent-encoding for a URL query (everything but the unreserved characters).
fn url_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The « retour d'expérience » issue form (.github/ISSUE_TEMPLATE/retour-experience.yml), filled with this PC.
/// The player reads it and sends it himself on GitHub.
fn feedback_url(app: &App) -> String {
    let f = app.facts.lock().unwrap().clone();
    let hw = hardware_of(f.as_ref());
    let p = &app.paths;
    let s = Settings::load(p);
    let gib = |b: f64| format!("{} Go", (b / (1u64 << 30) as f64).round());
    let mut q: Vec<(&str, String)> = vec![("template", "retour-experience.yml".into())];
    let gpu = f.as_ref().map(|f| f.gpu_name.clone()).unwrap_or_default();
    q.push(("title", format!("[retour] {}", if gpu.is_empty() { "ma config".into() } else { profile::short_gpu(&gpu) })));
    if let Some(f) = &f {
        q.push(("gpu", f.gpu_name.clone()));
        if f.vram > 0.0 {
            q.push(("vram", gib(f.vram)));
        }
        q.push(("cpu", format!("{} ({} fils)", f.cpu_name, f.threads)));
        q.push(("windows", f.windows.clone()));
    }
    if hw.ram_bytes > 0 {
        q.push(("ram", gib(hw.ram_bytes as f64)));
    }
    q.push(("version", format!("VIctory S {VERSION} ({})", if p.packaged { "version téléchargée" } else { "compilé depuis les sources" })));
    if let Ok(r) = Reglages::load(p) {
        let prof = settings::profile(&r, &hw);
        q.push(("profil", profile_text(&prof, &s, &r, r.dlss_dir_ok(&s.str("dlss_dir")))));
    }
    let query: Vec<String> = q.iter().filter(|(_, v)| !v.trim().is_empty()).map(|(k, v)| format!("{k}={}", url_encode(v))).collect();
    format!("https://github.com/sagouindupm/Mandy/issues/new?{}", query.join("&"))
}

// ------------------------------------------------------------------ --preview: the README's screenshots

#[tauri::command]
fn preview_mode(app: State<'_, App>) -> bool {
    app.preview.is_some()
}

/// Saves the window as it is now in the preview folder, as launcher-<name>.png.
#[tauri::command]
async fn preview_shot(handle: AppHandle, name: String) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let app = handle.state::<App>();
        let dir = app.preview.clone().ok_or("Pas en mode --preview.")?;
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
            return Err(format!("Nom invalide : {name}"));
        }
        let w = handle.get_webview_window("main").ok_or("Pas de fenêtre.")?;
        let hwnd = w.hwnd().map_err(|e| e.to_string())?;
        let out = dir.join(format!("launcher-{name}.png"));
        sys::capture_window(hwnd.0 as isize, &out).map(|p| p.display().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn preview_done(handle: AppHandle) {
    handle.exit(0);
}

// ------------------------------------------------------------------ --print-env

/// `--print-env <file> [--dlss <dir> <mode>] [--ram-gb N] [--vram-gb N] [--threads N] [--gpu <name>]`: writes the
/// VICTORY_ variables and PATH the game would get (sorted, NAME=value), for comparing with series-s/reglages.ps1
/// (scripts/check-profils.ps1). What is not given is read from this PC. The older form
/// `<file> <dlss_dir> <dlss_mode> [<ram_gb>]` still works. No window.
fn print_env(args: &[String]) -> Result<(), String> {
    let Some(file) = args.first() else { return Err("--print-env <fichier>".into()) };
    let paths = Paths::discover();
    let mut s = Settings::load(&paths);
    let (mut ram, mut vram, mut threads, mut gpu): (Option<u64>, Option<f64>, Option<usize>, Option<String>) = (None, None, None, None);
    let rest = &args[1..];
    if rest.first().is_some_and(|a| !a.starts_with("--")) {
        if let [dir, mode, ..] = rest {
            s.set("dlss_dir", dir.as_str());
            s.set("dlss_mode", mode.as_str());
        }
        ram = rest.get(2).and_then(|g| g.parse().ok());
    } else {
        let mut it = rest.iter();
        while let Some(a) = it.next() {
            let mut next = || it.next().cloned().ok_or(format!("{a} : valeur manquante"));
            match a.as_str() {
                "--dlss" => {
                    s.set("dlss_dir", next()?);
                    s.set("dlss_mode", next()?);
                }
                "--ram-gb" => ram = next()?.parse().ok(),
                "--vram-gb" => vram = next()?.parse().ok(),
                "--threads" => threads = next()?.parse().ok(),
                "--gpu" => gpu = Some(next()?),
                other => return Err(format!("Option inconnue : {other}")),
            }
        }
    }
    let facts = if gpu.is_some() && vram.is_some() { None } else { Some(sys::facts()) };
    let mut hw = hardware_of(facts.as_ref());
    if let Some(g) = ram {
        hw.ram_bytes = g << 30;
    }
    if let Some(v) = vram {
        hw.vram_bytes = (v * (1u64 << 30) as f64) as u64;
    }
    if let Some(t) = threads {
        hw.threads = t;
    }
    if let Some(g) = gpu {
        hw.gpu_name = g;
    }
    let plan = game_env(&paths, &Reglages::load(&paths)?, &s, &hw);
    let mut env: std::collections::BTreeMap<String, String> =
        std::env::vars().filter(|(k, _)| k.starts_with("VICTORY_") || k.eq_ignore_ascii_case("PATH")).collect();
    if let Some(dir) = &plan.path_prepend {
        let key = env.keys().find(|k| k.eq_ignore_ascii_case("PATH")).cloned().unwrap_or_else(|| "Path".into());
        let old = env.get(&key).cloned().unwrap_or_default();
        env.insert(key, format!("{};{old}", dir.display()));
    }
    env.extend(plan.set.iter().cloned());
    for k in &plan.remove {
        env.remove(k);
    }
    let text: String = env.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
    std::fs::write(file, text).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--print-env") {
        if let Err(e) = print_env(&args[i + 1..]) {
            let _ = std::fs::write(std::env::temp_dir().join("sagouindupm-print-env.txt"), e);
        }
        return;
    }
    // --preview <dir>: an empty home of its own (no screenshot of the game in the background, no path of the
    // player's, nothing done yet), the window shown without taking the focus.
    let preview = args.iter().position(|a| a == "--preview").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    if let Some(dir) = &preview {
        let home = std::env::temp_dir().join("victory-s-preview-home");
        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::create_dir_all(dir);
        // SAFETY: nothing else runs yet, no thread reads the environment.
        unsafe { std::env::set_var("SAGOUINDUPM_HOME", &home) };
    }
    let focused = preview.is_none();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            // The window of tauri.conf.json, created here so --preview can leave the focus to the program in front.
            if let Some(conf) = app.config().app.windows.iter().find(|w| w.label == "main") {
                let w = tauri::WebviewWindowBuilder::from_config(app.handle(), conf)?.focused(focused).build()?;
                if let Ok(hwnd) = w.hwnd() {
                    sys::paint_title_bar(hwnd.0);
                }
            }
            Ok(())
        })
        .manage(App { paths: Paths::discover(), jobs: Jobs::default(), facts: Mutex::new(None), last_game_log: Mutex::new(None), preview })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            facts,
            find_games,
            set_game,
            pick_game,
            pick_dlss,
            set_mode,
            run_job,
            stop_job,
            power,
            nvidia_panel,
            confirm_nv_cache,
            open,
            captures,
            capture_now,
            backup_saves,
            preview_mode,
            preview_shot,
            preview_done
        ])
        .on_window_event(|window, event| {
            // Closing the launcher does not leave a translation or a build running in the background.
            if let tauri::WindowEvent::Destroyed = event
                && let Some(app) = window.try_state::<App>()
                && app.jobs.running().is_some_and(|k| k != "play")
            {
                app.jobs.stop();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
