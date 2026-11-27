//! Long tasks (winget, cargo, victory shaders / warm, the game) as child processes. Their output goes, line by
//! line, to the window (event "job-line") and to a file in %LOCALAPPDATA%\Sagouindupm\journaux.

use std::io::{Read, Write};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::msg::Msg;
use crate::sys::{CREATE_NO_WINDOW, hidden};

#[derive(Default)]
pub struct Jobs {
    /// The running task: its kind and process id.
    pub current: Mutex<Option<(String, u32)>>,
}

#[derive(Serialize, Clone)]
pub struct JobLine {
    pub kind: String,
    pub line: String,
}

#[derive(Serialize, Clone)]
pub struct JobEvent {
    pub kind: String,
    pub code: Option<i32>,
}

pub struct Spec {
    pub kind: String,
    pub cmd: Command,
    pub log: PathBuf,
    /// The game: cmd.exe writes the log itself; the launcher follows the file.
    pub game: bool,
    /// The game: where the automatic screenshot goes, about three minutes in.
    pub capture_dir: Option<PathBuf>,
}

/// Seconds into a game before the automatic screenshot (past the menus and the loading).
const AUTO_CAPTURE_AFTER: u64 = 180;

fn emit_line(app: &AppHandle, kind: &str, line: &str) {
    let line = line.trim_end();
    if !line.trim().is_empty() {
        let _ = app.emit("job-line", JobLine { kind: kind.to_string(), line: line.to_string() });
    }
}

/// Splits a stream into lines on \n and \r (winget and cargo redraw their progress with \r).
fn pump(app: AppHandle, kind: String, mut from: impl Read, log: Arc<Mutex<std::fs::File>>) {
    let mut buf = [0u8; 8192];
    let mut pending: Vec<u8> = Vec::new();
    loop {
        let n = match from.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        if let Ok(mut f) = log.lock() {
            let _ = f.write_all(&buf[..n]);
        }
        for &b in &buf[..n] {
            if b == b'\n' || b == b'\r' {
                emit_line(&app, &kind, &String::from_utf8_lossy(&pending));
                pending.clear();
            } else {
                pending.push(b);
            }
        }
    }
    emit_line(&app, &kind, &String::from_utf8_lossy(&pending));
}

/// Follows the game's log: shows the once-a-second « fps » lines and errors only (a game writes a lot).
/// Takes one screenshot of the game about three minutes in (a background for the launcher's window).
fn follow(app: AppHandle, kind: String, log: PathBuf, capture_dir: Option<PathBuf>, done: Arc<Mutex<bool>>) {
    let started = std::time::Instant::now();
    let mut capture_dir = capture_dir;
    let mut pos = 0u64;
    let mut pending = String::new();
    loop {
        let finished = *done.lock().unwrap();
        if let Ok(mut f) = std::fs::File::open(&log) {
            use std::io::Seek;
            if f.seek(std::io::SeekFrom::Start(pos)).is_ok() {
                let mut chunk = Vec::new();
                if f.take(4 << 20).read_to_end(&mut chunk).is_ok() {
                    pos += chunk.len() as u64;
                    pending.push_str(&String::from_utf8_lossy(&chunk));
                    while let Some(i) = pending.find('\n') {
                        let line: String = pending.drain(..=i).collect();
                        let lower = line.to_lowercase();
                        if lower.contains(" fps,") || lower.contains("panic") || lower.contains("error") {
                            emit_line(&app, &kind, &line);
                        }
                    }
                }
            }
        }
        if finished {
            break;
        }
        if started.elapsed().as_secs() >= AUTO_CAPTURE_AFTER
            && let Some(dir) = capture_dir.take()
        {
            match crate::sys::capture_game(&dir) {
                Ok(file) => {
                    let _ = app.emit("capture", file.display().to_string());
                }
                Err(e) => emit_line(&app, &kind, &format!("Automatic screenshot failed: {e}")),
            }
        }
        std::thread::sleep(Duration::from_millis(700));
    }
}

impl Jobs {
    pub fn running(&self) -> Option<String> {
        self.current.lock().unwrap().as_ref().map(|(k, _)| k.clone())
    }

    /// Runs a task to its end (call from a blocking thread). Returns its exit code.
    pub fn run(&self, app: &AppHandle, mut spec: Spec) -> Result<i32, String> {
        {
            let cur = self.current.lock().unwrap();
            if let Some((k, _)) = cur.as_ref() {
                return Err(Msg::new("err.busy").with("task", Msg::new(&format!("job.{k}"))).into());
            }
        }
        let _ = app.emit("job-start", JobEvent { kind: spec.kind.clone(), code: None });
        spec.cmd.creation_flags(CREATE_NO_WINDOW).stdin(Stdio::null());
        let kind = spec.kind.clone();
        let code = if spec.game {
            spec.cmd.stdout(Stdio::null()).stderr(Stdio::null());
            let mut child = spec.cmd.spawn().map_err(|e| String::from(Msg::new("err.startFailed").with("error", e.to_string())))?;
            *self.current.lock().unwrap() = Some((kind.clone(), child.id()));
            let done = Arc::new(Mutex::new(false));
            let t = {
                let (app, kind, log, dir, done) = (app.clone(), kind.clone(), spec.log.clone(), spec.capture_dir.clone(), done.clone());
                std::thread::spawn(move || follow(app, kind, log, dir, done))
            };
            let status = child.wait();
            *done.lock().unwrap() = true;
            let _ = t.join();
            status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1)
        } else {
            let file = std::fs::File::create(&spec.log).map_err(|e| format!("{}: {e}", spec.log.display()))?;
            let log = Arc::new(Mutex::new(file));
            spec.cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
            let mut child = match spec.cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    // The log in English (for developers), the error for the window to translate.
                    emit_line(app, &kind, &format!("Could not start: {e}"));
                    let _ = app.emit("job-end", JobEvent { kind, code: Some(-1) });
                    return Err(Msg::new("err.startFailed").with("error", e.to_string()).into());
                }
            };
            *self.current.lock().unwrap() = Some((kind.clone(), child.id()));
            let out = child.stdout.take().unwrap();
            let err = child.stderr.take().unwrap();
            let t1 = {
                let (app, kind, log) = (app.clone(), kind.clone(), log.clone());
                std::thread::spawn(move || pump(app, kind, out, log))
            };
            let t2 = {
                let (app, kind, log) = (app.clone(), kind.clone(), log.clone());
                std::thread::spawn(move || pump(app, kind, err, log))
            };
            let status = child.wait();
            let _ = t1.join();
            let _ = t2.join();
            status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1)
        };
        *self.current.lock().unwrap() = None;
        let _ = app.emit("job-end", JobEvent { kind, code: Some(code) });
        Ok(code)
    }

    /// Stops the running task and everything it started.
    pub fn stop(&self) {
        if let Some((_, pid)) = self.current.lock().unwrap().as_ref() {
            let _ = hidden("taskkill.exe").args(["/PID", &pid.to_string(), "/T", "/F"]).status();
        }
    }
}
