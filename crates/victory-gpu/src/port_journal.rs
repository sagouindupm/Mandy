//! Renderer port #7: the submitting lane's last waits
//! (`VICTORY_PORT_JOURNAL=1`, off by default; the switch is
//! [`crate::watch::port_journal`]; credit `@avatarmanz` / `avatarmanz/VIctory-7900XTX` `34061929`).
//!
//! Two parts, both under the one switch:
//! - **The heaps' records**: a round of the heap-sync thread is asked for when
//!   the title submits ([`round_after`]), so it runs while the submission waits
//!   in the line, and the lane waits for that round (or a later one if a wait
//!   went past or a step wrote a heap's title memory).
//! - **The journal's lock** (`crate::watch`): the pages' notes are kept in 64
//!   sharded parts (`Notes`), each with its own lock.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// Whether the switch is on (`crate::watch::port_journal`).
pub(crate) fn on() -> bool {
    crate::watch::port_journal()
}

/// With rounds `asked` asked for and `done` done (one runs at a time, in order):
/// the number of a round that begins after now, and whether it has to be asked for.
pub(crate) fn round_after(asked: u64, done: u64) -> (u64, bool) {
    if asked >= done + 2 { (asked, false) } else { (asked + 1, true) }
}

thread_local! {
    /// On the lane: the round of the heaps' records the submission being
    /// prepared needs (0: none known, a round is asked for as before).
    static MARK: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// The lane is about to do a submission that needs round `mark`.
pub(crate) fn set_mark(mark: u64) {
    MARK.set(mark);
}

/// The round the submission being prepared needs; 0 if none is known.
fn mark() -> u64 {
    MARK.get()
}

/// The round every submission prepared from now on needs at least: one that
/// began after a step of a submission being prepared wrote, or may have
/// written, title memory of a heap of descriptors ([`prep_wrote`]).
static PREP_FLOOR: AtomicU64 = AtomicU64::new(0);

/// A step of the submission being prepared wrote, or may have written, title
/// memory of a heap of descriptors: every submission prepared after it waits
/// for a round begun after now.
pub(crate) fn prep_wrote() {
    PREP_FLOOR.fetch_max(crate::descriptor::round_mark(), Relaxed);
    STATS[PREP_WRITES].fetch_add(1, Relaxed);
}

/// The round the submission about to be prepared needs ([`mark`], at least
/// [`PREP_FLOOR`]); 0 if none is known (or the switch is off).
pub(crate) fn needed() -> u64 {
    if !on() {
        return 0;
    }
    match mark() {
        0 => {
            STATS[NO_MARK].fetch_add(1, Relaxed);
            0
        }
        m => m.max(PREP_FLOOR.load(Relaxed)),
    }
}

pub(crate) static STATS: [AtomicU64; 9] = [const { AtomicU64::new(0) }; 9];
pub(crate) const ROUNDS_ASKED: usize = 0;
pub(crate) const EARLY_ROUNDS: usize = 1;
const PREP_WRITES: usize = 2;
const HEAPS_NS_ON: usize = 3;
const HEAPS_NS_OFF: usize = 4;
const HEAPS_ON: usize = 5;
const HEAPS_OFF: usize = 6;
const NO_MARK: usize = 7;
pub(crate) const LATE_ROUNDS: usize = 8;

/// The lane waited `ns` for the heaps' records; `on`: with the switch.
pub(crate) fn heaps_waited(ns: u64, on: bool) {
    let (time, count) = if on { (HEAPS_NS_ON, HEAPS_ON) } else { (HEAPS_NS_OFF, HEAPS_OFF) };
    STATS[time].fetch_add(ns, Relaxed);
    STATS[count].fetch_add(1, Relaxed);
}

fn named() -> bool {
    #[cfg(feature = "diagnostics")]
    if crate::ab::bit("VICTORY_PORT_JOURNAL").is_some() {
        return true;
    }
    on()
}

/// Renderer port #7's line of the frame lines, per frame.
pub(crate) fn line(frames: f64) {
    let s: Vec<f64> = STATS.iter().map(|s| s.swap(0, Relaxed) as f64).collect();
    let journal = crate::watch::port_journal_counts();
    if !named() {
        return;
    }
    let f = frames.max(1.0);
    let (on_ms, off_ms) = (s[HEAPS_NS_ON] / 1e6 / f, s[HEAPS_NS_OFF] / 1e6 / f);
    tracing::info!(
        "renderer port 7 (VICTORY_PORT_JOURNAL=1) per frame: heaps' records: the lane waited {on_ms:.2} ms ({:.1} preparings) with \
         the switch, {off_ms:.2} ms ({:.1}) without; {:.1} rounds asked for at the title's submit, {:.1} preparings found theirs \
         asked earlier, {:.1} asked it at the preparing, {:.1} had no mark; {:.1} preparings wrote a heap's memory; journal: {}",
        s[HEAPS_ON] / f,
        s[HEAPS_OFF] / f,
        s[ROUNDS_ASKED] / f,
        s[EARLY_ROUNDS] / f,
        s[LATE_ROUNDS] / f,
        s[NO_MARK] / f,
        s[PREP_WRITES] / f,
        journal.per_frame(f),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_round_is_one_that_begins_after_now() {
        assert_eq!(round_after(0, 0), (1, true));
        assert_eq!(round_after(3, 2), (4, true));
        assert_eq!(round_after(5, 2), (5, false));
        assert_eq!(round_after(4, 2), (4, false));
        assert_eq!(round_after(7, 7), (8, true));
    }

    #[test]
    fn the_round_given_begins_after_the_call() {
        let (mut asked, mut done) = (0u64, 0u64);
        let mut began: std::collections::HashMap<u64, u64> = Default::default();
        let mut given: Vec<(u64, u64)> = Vec::new();
        let mut running: Option<u64> = None;
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for call in 1..5000u64 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            match seed % 3 {
                0 => {
                    let (round, ask) = round_after(asked, done);
                    if ask {
                        asked += 1;
                    }
                    given.push((call, round));
                }
                1 => {
                    if let Some(r) = running.take() {
                        done = r;
                    }
                }
                _ => {
                    if running.is_none() && asked > done {
                        running = Some(done + 1);
                        began.insert(done + 1, call);
                    }
                }
            }
        }
        for (call, round) in given {
            if let Some(&at) = began.get(&round) {
                assert!(at > call, "round {round} began at {at}, before the call at {call}");
            }
        }
    }
}
