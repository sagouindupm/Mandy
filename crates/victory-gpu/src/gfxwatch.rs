//! Black-window watchdog: which graphics stage a run reached last, and a
//! warning while no frame reaches the window.
//!
//! A window that stays black says nothing by itself: the device may be
//! missing, the swap chain never made, the title stuck before its first
//! Present (waiting for pipelines, a fence, a file), or frames may be shown
//! that hold nothing. Each stage below is logged the first time it is
//! reached, with the seconds since the device was made; a thread then looks
//! every few seconds and, while the window has had no frame for
//! [`QUIET_S`] seconds, logs the last stage reached and the counts that tell
//! the cases apart (title presents, host presents and their failures, black
//! frames, lists, draws, dispatches, shaders, pipelines and those still
//! being made, memory, whether the device is still there).
//!
//! `VICTORY_GFX_WATCH=<seconds>` sets how often it looks (default 5;
//! `0`: off). It only reads counters: no lock the title's threads take.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};
use std::time::Instant;

/// No frame for this long is a black window worth saying so.
const QUIET_S: u64 = 10;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(usize)]
pub(crate) enum Stage {
    Device,
    FirstShader,
    FirstPipeline,
    FirstList,
    FirstDraw,
    FirstDispatch,
    TitlePresent,
    SwapChain,
    FrameShown,
    FrameWithPicture,
}

const STAGES: usize = 10;
const NAMES: [&str; STAGES] = [
    "host D3D12 device made",
    "first shader translated",
    "first host pipeline made",
    "first title command list submitted",
    "first draw run on the host",
    "first dispatch run on the host",
    "first title Present",
    "swap chain made on the window",
    "first frame shown (host Present S_OK)",
    "first frame that is not black",
];

fn start() -> Instant {
    static T: OnceLock<Instant> = OnceLock::new();
    *T.get_or_init(Instant::now)
}

fn now_ms() -> u64 {
    start().elapsed().as_millis() as u64 + 1
}

/// When each stage was first reached (ms since the device, +1; 0: not yet).
static REACHED: [AtomicU64; STAGES] = [const { AtomicU64::new(0) }; STAGES];

static TITLE_PRESENTS: AtomicU64 = AtomicU64::new(0);
static SHOWN: AtomicU64 = AtomicU64::new(0);
static SHOWN_BLACK: AtomicU64 = AtomicU64::new(0);
static PRESENT_FAILED: AtomicU64 = AtomicU64::new(0);
static LAST_PRESENT_HR: AtomicU32 = AtomicU32::new(0);
static LAST_SHOWN_MS: AtomicU64 = AtomicU64::new(0);
static LISTS: AtomicU64 = AtomicU64::new(0);
static SHADERS_OK: AtomicU64 = AtomicU64::new(0);
static SHADERS_FAILED: AtomicU64 = AtomicU64::new(0);
static PIPELINES_OK: AtomicU64 = AtomicU64::new(0);
static PIPELINES_REFUSED: AtomicU64 = AtomicU64::new(0);

pub(crate) fn reached(stage: Stage) {
    let slot = &REACHED[stage as usize];
    if slot.load(Relaxed) != 0 {
        return;
    }
    let at = now_ms();
    if slot.compare_exchange(0, at, Relaxed, Relaxed).is_ok() {
        tracing::info!(
            stage = NAMES[stage as usize],
            seconds = format_args!("{:.1}", (at - 1) as f64 / 1000.0),
            "graphics stage reached"
        );
    }
}

pub(crate) fn title_present() {
    TITLE_PRESENTS.fetch_add(1, Relaxed);
    reached(Stage::TitlePresent);
}

/// A host Present: its HRESULT and, where the build looks, whether the
/// frame was black.
pub(crate) fn shown(hr: i32, black: Option<bool>) {
    if hr < 0 {
        PRESENT_FAILED.fetch_add(1, Relaxed);
        LAST_PRESENT_HR.store(hr as u32, Relaxed);
        return;
    }
    SHOWN.fetch_add(1, Relaxed);
    LAST_SHOWN_MS.store(now_ms(), Relaxed);
    crate::pacing::frame();
    reached(Stage::FrameShown);
    match black {
        Some(true) => {
            SHOWN_BLACK.fetch_add(1, Relaxed);
        }
        Some(false) => reached(Stage::FrameWithPicture),
        None => {}
    }
}

pub(crate) fn list_submitted(count: u32) {
    if count > 0 {
        LISTS.fetch_add(u64::from(count), Relaxed);
        reached(Stage::FirstList);
    }
}

pub(crate) fn shader(ok: bool) {
    if ok {
        SHADERS_OK.fetch_add(1, Relaxed);
        reached(Stage::FirstShader);
    } else {
        SHADERS_FAILED.fetch_add(1, Relaxed);
    }
}

pub(crate) fn pipeline(ok: bool) {
    if ok {
        PIPELINES_OK.fetch_add(1, Relaxed);
        reached(Stage::FirstPipeline);
    } else {
        PIPELINES_REFUSED.fetch_add(1, Relaxed);
    }
}

fn interval() -> u64 {
    static S: OnceLock<u64> = OnceLock::new();
    *S.get_or_init(|| {
        std::env::var("VICTORY_GFX_WATCH")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(5)
    })
}

/// The last stage reached, in the order a run goes through them.
fn last_stage() -> (&'static str, Vec<&'static str>) {
    let mut last = "none (no host device yet)";
    let mut missing = Vec::new();
    for (i, slot) in REACHED.iter().enumerate() {
        if slot.load(Relaxed) != 0 {
            last = NAMES[i];
        } else {
            missing.push(NAMES[i]);
        }
    }
    (last, missing)
}

/// What the counts point to, in words.
fn verdict(title: u64, shown: u64, black: u64, failed: u64, making: u64, removed: bool) -> &'static str {
    if removed {
        "the host device was removed: see the 'device removed' lines and DRED output"
    } else if title == 0 && making > 0 {
        "the title has not presented yet while host pipelines are still being made: \
         first-run pipeline compilation (driver cache cold); `victory warm --used` or waiting helps"
    } else if title == 0 {
        "the title has not presented yet: it is still loading or waits for something \
         (a fence, a file, a shader result); see the stack dumps (VICTORY_STACKS) for where"
    } else if shown == 0 && failed > 0 {
        "the title presents but every host Present fails: see the HRESULT"
    } else if shown == 0 {
        "the title presents but nothing reached the swap chain (no host copy of its front buffer, or an unsupported format)"
    } else if black == shown {
        "frames reach the window but all are black: draws are dropped (shaders that do not translate, pipelines refused or not made yet)"
    } else {
        "frames were shown before and stopped: the title or a wait of ours is stuck"
    }
}

fn process_private_mb() -> u64 {
    use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut c: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    c.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    // SAFETY: a sized structure for our own process.
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), (&raw mut c).cast(), c.cb) };
    if ok == 0 { 0 } else { c.PrivateUsage as u64 >> 20 }
}

fn look() {
    // (Draws and dispatches are counted where they run; their first time is
    // taken from those counts, at this look's resolution.)
    if crate::draw::drawn() > 0 {
        reached(Stage::FirstDraw);
    }
    if crate::draw::dispatched() > 0 {
        reached(Stage::FirstDispatch);
    }
    let quiet_ms = now_ms().saturating_sub(LAST_SHOWN_MS.load(Relaxed).max(1));
    let shown = SHOWN.load(Relaxed);
    if shown > 0 && quiet_ms < QUIET_S * 1000 {
        return;
    }
    if shown == 0 && now_ms() < QUIET_S * 1000 {
        return;
    }
    let (last, missing) = last_stage();
    let title = TITLE_PRESENTS.load(Relaxed);
    let black = SHOWN_BLACK.load(Relaxed);
    let failed = PRESENT_FAILED.load(Relaxed);
    let making = crate::pipeline::making();
    let host = crate::host::host();
    // SAFETY: a plain query on a live device.
    let removed = host.is_some_and(|h| unsafe { h.device.GetDeviceRemovedReason() }.is_err());
    let [budget, used, shared_budget, shared_used] = host.and_then(|h| h.video_memory()).unwrap_or([0; 4]);
    tracing::warn!(
        seconds_without_a_frame = quiet_ms / 1000,
        last_stage = last,
        not_reached = missing.join("; "),
        title_presents = title,
        host_presents_ok = shown,
        host_presents_black = black,
        host_presents_failed = failed,
        last_present_hresult = format_args!("{:#010x}", LAST_PRESENT_HR.load(Relaxed)),
        lists = LISTS.load(Relaxed),
        draws = crate::draw::drawn(),
        dispatches = crate::draw::dispatched(),
        shaders_ok = SHADERS_OK.load(Relaxed),
        shaders_failed = SHADERS_FAILED.load(Relaxed),
        pipelines_ok = PIPELINES_OK.load(Relaxed),
        pipelines_refused = PIPELINES_REFUSED.load(Relaxed),
        pipelines_being_made = making,
        vram_mb = format_args!("{} of {} budget", used >> 20, budget >> 20),
        shared_mb = format_args!("{} of {} budget", shared_used >> 20, shared_budget >> 20),
        private_mb = process_private_mb(),
        device_removed = removed,
        "BLACK WINDOW: {}",
        verdict(title, shown, black, failed, making, removed)
    );
}

/// Starts the watchdog once (with the host device).
pub(crate) fn begin() {
    let _ = start();
    let every = interval();
    if every == 0 {
        return;
    }
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = std::thread::Builder::new().name("victory-gfxwatch".into()).spawn(move || {
            let mut paced = Instant::now();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(every));
                look();
                if paced.elapsed().as_secs() >= crate::pacing::WINDOW_S {
                    paced = Instant::now();
                    crate::pacing::report();
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdicts_tell_the_cases_apart() {
        assert!(verdict(0, 0, 0, 0, 5, false).contains("pipeline compilation"));
        assert!(verdict(0, 0, 0, 0, 0, false).contains("not presented yet"));
        assert!(verdict(10, 0, 0, 10, 0, false).contains("HRESULT"));
        assert!(verdict(10, 0, 0, 0, 0, false).contains("nothing reached"));
        assert!(verdict(10, 10, 10, 0, 0, false).contains("all are black"));
        assert!(verdict(10, 10, 2, 0, 0, false).contains("stopped"));
        assert!(verdict(10, 10, 2, 0, 0, true).contains("removed"));
    }

    #[test]
    fn a_stage_is_kept_from_its_first_time() {
        reached(Stage::FirstDraw);
        let first = REACHED[Stage::FirstDraw as usize].load(Relaxed);
        assert_ne!(first, 0);
        std::thread::sleep(std::time::Duration::from_millis(3));
        reached(Stage::FirstDraw);
        assert_eq!(REACHED[Stage::FirstDraw as usize].load(Relaxed), first);
        assert!(last_stage().1.iter().all(|m| *m != NAMES[Stage::FirstDraw as usize]));
    }

    #[test]
    fn a_failed_present_is_counted_and_not_shown() {
        let before = SHOWN.load(Relaxed);
        shown(0x887A0005u32 as i32, None);
        assert_eq!(SHOWN.load(Relaxed), before);
        assert_eq!(LAST_PRESENT_HR.load(Relaxed), 0x887A0005);
        shown(0, Some(true));
        assert_eq!(SHOWN.load(Relaxed), before + 1);
    }
}
