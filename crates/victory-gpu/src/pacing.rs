//! Frame pacing: how evenly frames reach the window, not only how many.
//!
//! An average of 15 fps can hide a frame of two seconds. Every host Present
//! that succeeds records the time since the one before; every
//! [`WINDOW_S`] seconds the watchdog thread logs, over that window, the
//! average fps, the 1% and 0.1% lows (the fps of the slowest 1% and 0.1% of
//! frames, averaged), the median, P95 and P99 frame times, the longest
//! frame, and how many frames took more than 100 ms, 250 ms and a second;
//! the counts since the start follow, so a whole session can be read from
//! its last line.
//!
//! `VICTORY_PACING=0` turns it off. Recording takes one short lock on the
//! presenting thread; the sorting happens on the watchdog's thread.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Instant;

/// Seconds a pacing line covers.
pub(crate) const WINDOW_S: u64 = 10;

struct State {
    last: Option<Instant>,
    /// Frame times of the current window, in microseconds.
    frames: Vec<u32>,
}

static STATE: Mutex<State> = Mutex::new(State { last: None, frames: Vec::new() });

/// Since the start: frames, and those over 100 ms, 250 ms and 1 s.
static TOTAL: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
/// Since the start: the longest frame, in microseconds.
static LONGEST: AtomicU64 = AtomicU64::new(0);

pub(crate) fn on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("VICTORY_PACING").map_or(true, |v| v.trim() != "0"))
}

/// A frame reached the window.
pub(crate) fn frame() {
    if !on() {
        return;
    }
    let now = Instant::now();
    let Ok(mut s) = STATE.lock() else { return };
    if let Some(last) = s.last.replace(now) {
        let us = now.duration_since(last).as_micros().min(u128::from(u32::MAX)) as u32;
        s.frames.push(us);
        count(us);
    }
}

fn count(us: u32) {
    TOTAL[0].fetch_add(1, Relaxed);
    for (i, limit) in [(1, 100_000), (2, 250_000), (3, 1_000_000)] {
        if us > limit {
            TOTAL[i].fetch_add(1, Relaxed);
        }
    }
    LONGEST.fetch_max(u64::from(us), Relaxed);
}

/// What a window of frame times says.
#[derive(Debug, PartialEq)]
pub(crate) struct Summary {
    pub frames: usize,
    pub fps: f64,
    pub low_1: f64,
    pub low_01: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    pub over_100: usize,
    pub over_250: usize,
    pub over_1000: usize,
}

/// The fps of the slowest `share` of the frames: their count over their time.
fn low(sorted: &[u32], share: f64) -> f64 {
    let n = ((sorted.len() as f64 * share).ceil() as usize).clamp(1, sorted.len());
    let slow = &sorted[sorted.len() - n..];
    let us: u64 = slow.iter().map(|&t| u64::from(t)).sum();
    if us == 0 { 0.0 } else { n as f64 * 1e6 / us as f64 }
}

fn at(sorted: &[u32], q: f64) -> f64 {
    let i = ((sorted.len() as f64 * q).ceil() as usize).clamp(1, sorted.len()) - 1;
    f64::from(sorted[i]) / 1e3
}

pub(crate) fn summarize(mut frames: Vec<u32>) -> Option<Summary> {
    if frames.is_empty() {
        return None;
    }
    frames.sort_unstable();
    let us: u64 = frames.iter().map(|&t| u64::from(t)).sum();
    let over = |limit: u32| frames.iter().filter(|&&t| t > limit).count();
    Some(Summary {
        frames: frames.len(),
        fps: if us == 0 { 0.0 } else { frames.len() as f64 * 1e6 / us as f64 },
        low_1: low(&frames, 0.01),
        low_01: low(&frames, 0.001),
        median_ms: at(&frames, 0.5),
        p95_ms: at(&frames, 0.95),
        p99_ms: at(&frames, 0.99),
        max_ms: f64::from(*frames.last().unwrap()) / 1e3,
        over_100: over(100_000),
        over_250: over(250_000),
        over_1000: over(1_000_000),
    })
}

/// Logs the window that ended and starts the next (from the watchdog).
pub(crate) fn report() {
    if !on() {
        return;
    }
    let frames = match STATE.lock() {
        Ok(mut s) => std::mem::take(&mut s.frames),
        Err(_) => return,
    };
    let Some(s) = summarize(frames) else { return };
    let [total, t100, t250, t1000] = [0, 1, 2, 3].map(|i| TOTAL[i].load(Relaxed));
    tracing::info!(
        "frame pacing, last {WINDOW_S} s: {:.1} fps average, 1% low {:.1} fps, 0.1% low {:.1} fps; frame time median {:.1} ms, \
         P95 {:.1} ms, P99 {:.1} ms, longest {:.1} ms; {} frames, {} over 100 ms, {} over 250 ms, {} over 1 s; \
         since the start: {total} frames, {t100} over 100 ms, {t250} over 250 ms, {t1000} over 1 s, longest {:.1} ms (VICTORY_PACING=0: off)",
        s.fps,
        s.low_1,
        s.low_01,
        s.median_ms,
        s.p95_ms,
        s.p99_ms,
        s.max_ms,
        s.frames,
        s.over_100,
        s.over_250,
        s.over_1000,
        LONGEST.load(Relaxed) as f64 / 1e3
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_frames_have_lows_equal_to_the_average() {
        let s = summarize(vec![50_000; 200]).unwrap();
        assert!((s.fps - 20.0).abs() < 1e-9);
        assert!((s.low_1 - 20.0).abs() < 1e-9);
        assert_eq!(s.over_100, 0);
        assert!((s.p99_ms - 50.0).abs() < 1e-9);
    }

    #[test]
    fn one_long_stall_shows_in_the_lows_and_counts_not_in_the_median() {
        let mut frames = vec![50_000; 199];
        frames.push(2_000_000);
        let s = summarize(frames).unwrap();
        // 200 frames in 11.95 s.
        assert!((s.fps - 200.0 / 11.95).abs() < 1e-6);
        // The slowest 1% is two frames: the stall and one of 50 ms.
        assert!((s.low_1 - 2.0 / 2.05).abs() < 1e-6);
        assert!((s.low_01 - 0.5).abs() < 1e-9);
        assert!((s.median_ms - 50.0).abs() < 1e-9);
        assert!((s.max_ms - 2000.0).abs() < 1e-9);
        assert_eq!((s.over_100, s.over_250, s.over_1000), (1, 1, 1));
    }

    #[test]
    fn nothing_to_say_without_frames() {
        assert_eq!(summarize(Vec::new()), None);
    }
}
