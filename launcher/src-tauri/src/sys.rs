//! Facts about the PC and the small system actions (power plan, opening Windows tools, finding the game).
//! Everything here is called from a blocking thread, never from the window's thread.

use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::paths::{victory_dir, which};

/// No console window for the helper programs (powershell, powercfg, taskkill...).
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn hidden(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut c = Command::new(program);
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

// ------------------------------------------------------------------ time, memory, power line

/// Local time as (year, month, day, hour, minute, second).
pub fn local_time() -> (u16, u16, u16, u16, u16, u16) {
    // SAFETY: GetLocalTime only writes the structure it is given.
    let t = unsafe {
        let mut t = std::mem::zeroed::<windows_sys::Win32::Foundation::SYSTEMTIME>();
        windows_sys::Win32::System::SystemInformation::GetLocalTime(&mut t);
        t
    };
    (t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Total RAM in bytes (0 if unknown).
pub fn total_ram() -> u64 {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: dwLength is set as the API requires; the call only writes the structure.
    unsafe {
        let mut m = std::mem::zeroed::<MEMORYSTATUSEX>();
        m.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut m) != 0 { m.ullTotalPhys } else { 0 }
    }
}

/// "online" (on mains), "offline" (on battery) or "unknown".
pub fn power_line() -> &'static str {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    // SAFETY: the call only writes the structure.
    let st = unsafe {
        let mut s = std::mem::zeroed::<SYSTEM_POWER_STATUS>();
        if GetSystemPowerStatus(&mut s) == 0 {
            return "unknown";
        }
        s
    };
    match st.ACLineStatus {
        0 => "offline",
        1 => "online",
        _ => "unknown",
    }
}

fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() => total += e.metadata().map(|m| m.len()).unwrap_or(0),
                _ => {}
            }
        }
    }
    total
}

/// The title bar in the sidebar's near-black with white text (Windows 11; ignored by Windows 10), instead of the
/// user's accent colour.
pub fn paint_title_bar(hwnd: *mut std::ffi::c_void) {
    use windows_sys::Win32::Graphics::Dwm::{DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DwmSetWindowAttribute};
    // COLORREF is 0x00BBGGRR: #111111 and white.
    let (back, text): (u32, u32) = (0x0011_1111, 0x00FF_FFFF);
    for (attr, color) in [(DWMWA_CAPTION_COLOR, back), (DWMWA_BORDER_COLOR, back), (DWMWA_TEXT_COLOR, text)] {
        // SAFETY: a valid window handle and a 4-byte value, as the attribute expects.
        unsafe {
            DwmSetWindowAttribute(hwnd, attr as u32, (&color as *const u32).cast(), 4);
        }
    }
}

// ------------------------------------------------------------------ facts

/// The slow checks, in one PowerShell run (WMI, vswhere, the driver's registry, nvidia-smi, powercfg).
const FACTS_PS: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$f = [ordered]@{}
$vc = $null
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (Test-Path $vswhere) { $vc = & $vswhere -products * -latest -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null | Select-Object -First 1 }
if (-not $vc) { $l = Get-Command link.exe | Where-Object { $_.Source -match 'MSVC' } | Select-Object -First 1; if ($l) { $vc = $l.Source } }
$f.vcTools = [string]$vc
$f.winSdk = [bool](Get-ChildItem (Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\Lib\*\um\x64\kernel32.lib') | Select-Object -First 1)
$cs = Get-CimInstance Win32_ComputerSystem
$f.autoPagefile = if ($cs) { [bool]$cs.AutomaticManagedPagefile } else { $null }
$gpus = @(Get-CimInstance Win32_VideoController | Where-Object { $_.Name -notmatch 'Basic Display|Remote|Virtual|Parsec|Meta Virtual' })
$gpu = $gpus | Where-Object { $_.Name -match 'NVIDIA' } | Select-Object -First 1
if (-not $gpu) { $gpu = $gpus | Sort-Object AdapterRAM -Descending | Select-Object -First 1 }
$f.gpuName = if ($gpu) { [string]$gpu.Name } else { '' }
$vram = [double]0
Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0*' | ForEach-Object {
    $q = $_.'HardwareInformation.qwMemorySize'
    if ($_.DriverDesc -eq $f.gpuName -and $q) { try { $vram = [double]$q } catch { } }
}
if (-not $vram -and $gpu) { $vram = [double]$gpu.AdapterRAM }
$f.vram = $vram
$f.bar1Mb = 0
$smi = (Get-Command nvidia-smi.exe | Select-Object -First 1).Source
if (-not $smi) { $c = Join-Path $env:SystemRoot 'System32\nvidia-smi.exe'; if (Test-Path $c) { $smi = $c } }
if ($smi) {
    $inBar = $false
    foreach ($line in (& $smi -q -d MEMORY 2>$null)) {
        if ($line -match 'BAR1') { $inBar = $true; continue }
        if ($inBar -and $line -match 'Total\s*:\s*(\d+)\s*MiB') { $f.bar1Mb = [int]$Matches[1]; break }
    }
}
$act = (& powercfg.exe /getactivescheme 2>$null) -join ' '
$f.schemeGuid = if ($act -match '([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})') { $Matches[1].ToLower() } else { '' }
$f.schemeName = if ($act -match '\(([^)]+)\)\s*$') { $Matches[1] } else { '' }
$overlay = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Power\User\PowerSchemes').ActiveOverlayAcPowerScheme
$f.schemeGood = ($f.schemeGuid -in @('8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c', 'e9a42b02-d5df-448d-aa00-03f14749eb61')) -or ($overlay -eq 'ded574b5-45a0-4f42-8737-46345c09c238')
$f.battery = [bool](Get-CimInstance Win32_Battery | Select-Object -First 1)
$f | ConvertTo-Json -Compress
"#;

#[derive(Deserialize, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    #[serde(default)]
    pub cargo: String,
    #[serde(default)]
    pub winget: String,
    #[serde(default)]
    pub vc_tools: String,
    #[serde(default)]
    pub win_sdk: bool,
    #[serde(default)]
    pub auto_pagefile: Option<bool>,
    #[serde(default)]
    pub gpu_name: String,
    #[serde(default)]
    pub nvidia: bool,
    #[serde(default)]
    pub vram: f64,
    #[serde(default)]
    pub bar1_mb: u32,
    #[serde(default)]
    pub scheme_guid: String,
    #[serde(default)]
    pub scheme_name: String,
    #[serde(default)]
    pub scheme_good: bool,
    #[serde(default)]
    pub battery: bool,
    #[serde(default)]
    pub dx_cache: u64,
}

/// Runs a PowerShell script (Windows PowerShell 5.1, as the console launcher) and returns its standard output.
pub fn powershell(script: &str) -> Result<String, String> {
    let utf16: Vec<u8> = script.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    let out = hidden("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &base64(&utf16)])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("powershell: {e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// Rust: cargo in PATH, or where rustup installs it (not yet in PATH right after the installation).
pub fn find_cargo() -> Option<PathBuf> {
    which("cargo.exe").or_else(|| {
        let homes = [std::env::var_os("CARGO_HOME").map(PathBuf::from), std::env::var_os("USERPROFILE").map(|u| PathBuf::from(u).join(".cargo"))];
        homes.into_iter().flatten().map(|h| h.join("bin").join("cargo.exe")).find(|p| p.is_file())
    })
}

pub fn find_winget() -> Option<PathBuf> {
    which("winget.exe").or_else(|| {
        let p = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join(r"Microsoft\WindowsApps\winget.exe");
        p.exists().then_some(p)
    })
}

pub fn facts() -> Facts {
    let mut f: Facts = powershell(FACTS_PS)
        .ok()
        .and_then(|out| out.lines().rev().find(|l| l.trim_start().starts_with('{')).map(str::to_owned))
        .and_then(|l| serde_json::from_str(&l).ok())
        .unwrap_or_default();
    f.cargo = find_cargo().map(|p| p.display().to_string()).unwrap_or_default();
    f.winget = find_winget().map(|p| p.display().to_string()).unwrap_or_default();
    f.nvidia = f.gpu_name.to_uppercase().contains("NVIDIA");
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        f.dx_cache = dir_size(&PathBuf::from(local).join("NVIDIA").join("DXCache"));
    }
    f
}

/// « victory warm » done once (cache of victory.exe, VICTORY_CACHE_DIR honoured).
pub fn warm_done() -> bool {
    victory_dir("VICTORY_CACHE_DIR", "cache").join("pipelines").join("warm-done.txt").is_file()
}

// ------------------------------------------------------------------ power plan

pub fn active_scheme() -> String {
    let out = hidden("powercfg.exe").arg("/getactivescheme").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    out.split(|c: char| !(c.is_ascii_hexdigit() || c == '-'))
        .find(|w| w.len() == 36 && w.matches('-').count() == 4)
        .map(|w| w.to_lowercase())
        .unwrap_or_default()
}

pub fn set_scheme(scheme: &str) -> bool {
    hidden("powercfg.exe").args(["/setactive", scheme]).status().map(|s| s.success()).unwrap_or(false)
}

// ------------------------------------------------------------------ opening things

/// ShellExecute through cmd's start: folders, files, web pages, ms-settings: and programs that ask for
/// administrator rights (SystemPropertiesPerformance.exe).
pub fn shell_open(target: &str) -> Result<(), String> {
    hidden("cmd.exe")
        .raw_arg(format!("/c start \"\" \"{target}\""))
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{target}: {e}"))
}

pub fn open_nvidia_panel() -> bool {
    let exe = PathBuf::from(std::env::var_os("ProgramFiles").unwrap_or_default()).join(r"NVIDIA Corporation\Control Panel Client\nvcplui.exe");
    if exe.is_file() {
        return Command::new(exe).spawn().is_ok();
    }
    // Store version of the panel (DCH drivers).
    let out = powershell(
        "$p = Get-AppxPackage -Name 'NVIDIACorp.NVIDIAControlPanel' -ErrorAction SilentlyContinue | Select-Object -First 1\n\
         if ($p) { Start-Process explorer.exe \"shell:AppsFolder\\$($p.PackageFamilyName)!NVIDIACorp.NVIDIAControlPanel\"; 'ok' }",
    )
    .unwrap_or_default();
    out.contains("ok")
}

// ------------------------------------------------------------------ screenshots of the game

/// The client area of victory.exe's window at its native resolution, as a PNG. PrintWindow first (works when
/// the window is covered); if that comes back black (some D3D12 swap chains), a copy of the screen.
const CAPTURE_PS: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SvCap {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
}
'@
[SvCap]::SetProcessDPIAware() | Out-Null
$p = Get-Process victory -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $p) { 'ERR:Le jeu ne tourne pas.'; exit }
$h = $p.MainWindowHandle
$r = New-Object SvCap+RECT
[SvCap]::GetClientRect($h, [ref]$r) | Out-Null
$w = $r.R - $r.L; $hg = $r.B - $r.T
if ($w -le 0 -or $hg -le 0) { 'ERR:La fenêtre du jeu est réduite.'; exit }
$bmp = New-Object System.Drawing.Bitmap $w, $hg
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
[SvCap]::PrintWindow($h, $hdc, 3) | Out-Null
$g.ReleaseHdc($hdc)
$lit = 0
for ($y = 1; $y -lt 10; $y++) { for ($x = 1; $x -lt 10; $x++) {
    $c = $bmp.GetPixel([int]($w * $x / 10), [int]($hg * $y / 10)); if ($c.R + $c.G + $c.B -gt 30) { $lit++ } } }
if ($lit -lt 3) {
    $pt = New-Object SvCap+POINT
    [SvCap]::ClientToScreen($h, [ref]$pt) | Out-Null
    $g.CopyFromScreen($pt.X, $pt.Y, 0, 0, (New-Object System.Drawing.Size $w, $hg))
}
$g.Dispose()
$bmp.Save('__OUT__', [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
"OK:$w x $hg"
"#;

/// Saves a screenshot of the running game in `dir`; returns the file.
pub fn capture_game(dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let (y, mo, d, h, mi, s) = local_time();
    let out = dir.join(format!("capture_{y:04}-{mo:02}-{d:02}_{h:02}{mi:02}{s:02}.png"));
    let script = CAPTURE_PS.replace("__OUT__", &out.display().to_string().replace('\'', "''"));
    let res = powershell(&script)?;
    let last = res.lines().map(str::trim).rfind(|l| l.starts_with("OK:") || l.starts_with("ERR:")).unwrap_or("ERR:Capture impossible.");
    match last.strip_prefix("ERR:") {
        Some(err) => Err(err.to_string()),
        None if out.is_file() => Ok(out),
        None => Err("Capture impossible.".into()),
    }
}

/// Pixel area of a PNG from its header (0 if not a PNG).
fn png_area(path: &Path) -> u64 {
    let mut head = [0u8; 24];
    let ok = std::fs::File::open(path).and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head)).is_ok();
    if !ok || &head[1..4] != b"PNG" {
        return 0;
    }
    let w = u32::from_be_bytes(head[16..20].try_into().unwrap()) as u64;
    let h = u32::from_be_bytes(head[20..24].try_into().unwrap()) as u64;
    w * h
}

/// The player's screenshots (PNG, JPEG), the largest then the newest first.
pub fn list_captures(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut found: Vec<(u64, std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg")))
        .map(|p| {
            let t = std::fs::metadata(&p).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
            (png_area(&p), t, p)
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    found.into_iter().map(|(_, _, p)| p).collect()
}

// ------------------------------------------------------------------ finding the game

/// game_scarlett_master.exe in Downloads, Desktop, Documents, the usual game folders and the drive roots,
/// three folder levels deep at most.
pub fn find_games() -> Vec<String> {
    const NAME: &str = "game_scarlett_master.exe";
    const SKIP: &[&str] = &[
        "windows", "$recycle.bin", "system volume information", "windowsapps", "appdata", "programdata", "recovery",
        "node_modules", ".git", "windows kits", "microsoft visual studio", "perflogs", "config.msi", ".cargo", ".rustup",
    ];
    let mut starts: Vec<PathBuf> = Vec::new();
    if let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        for sub in ["Downloads", "Desktop", "Documents", "OneDrive\\Desktop", "OneDrive\\Documents"] {
            starts.push(profile.join(sub));
        }
    }
    for letter in b'C'..=b'Z' {
        let root = PathBuf::from(format!("{}:\\", letter as char));
        if !root.exists() {
            continue;
        }
        for sub in ["Games", "Jeux", "XboxGames", "SteamLibrary", "Epic Games", "Program Files", "Program Files (x86)"] {
            starts.push(root.join(sub));
        }
        starts.push(root);
    }
    let mut found: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for start in starts.into_iter().filter(|p| p.is_dir()) {
        let mut queue = std::collections::VecDeque::from([(0u32, start)]);
        while let Some((depth, dir)) = queue.pop_front() {
            if !seen.insert(dir.to_string_lossy().to_lowercase()) {
                continue;
            }
            let exe = dir.join(NAME);
            if exe.is_file() {
                let s = exe.display().to_string();
                if !found.contains(&s) {
                    found.push(s);
                }
            }
            if depth >= 3 {
                continue;
            }
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let Ok(t) = e.file_type() else { continue };
                // Not into junctions and links (file_type does not follow them).
                if !t.is_dir() || t.is_symlink() {
                    continue;
                }
                let name = e.file_name().to_string_lossy().to_lowercase();
                if SKIP.contains(&name.as_str()) {
                    continue;
                }
                queue.push_back((depth + 1, e.path()));
            }
        }
    }
    found
}

/// Copies a folder recursively.
pub fn copy_dir(from: &Path, to: &Path) -> std::io::Result<u64> {
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let dst = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            n += copy_dir(&e.path(), &dst)?;
        } else {
            std::fs::copy(e.path(), &dst)?;
            n += 1;
        }
    }
    Ok(n)
}
