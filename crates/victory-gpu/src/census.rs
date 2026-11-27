//! `VICTORY_SKIP_WC_BACK=1` / `VICTORY_SKIP_WC_COMPARE=1` (ported from fxceless/victory-fork#868):
//! tracks `XMemVirtualAlloc` console protection bits (specifically write-combining `0x400`) so
//! shader writes to write-combining card buffers can opt-in skip copy-back / compare.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

struct Reservation {
    len: usize,
    _kind: u32,
    _flags: u64,
    protect: u32,
}

static RESERVATIONS: Mutex<BTreeMap<usize, Reservation>> = Mutex::new(BTreeMap::new());

/// The runtime reserved `[at, at + len)` for the title (`XMemVirtualAlloc`
/// at an address of its choice), with the title's own console `protect` bits.
pub fn title_memory_reserved(at: usize, len: usize, kind: u32, flags: u64, protect: u32) {
    if !skip_wc_back() || at == 0 || len == 0 {
        return;
    }
    RESERVATIONS.lock().unwrap_or_else(|e| e.into_inner()).insert(
        at,
        Reservation {
            len,
            _kind: kind,
            _flags: flags,
            protect,
        },
    );
}

/// Minimum copy size eligible for `VICTORY_SKIP_WC_BACK` / `VICTORY_SKIP_WC_COMPARE`
/// (`VICTORY_SKIP_WC_MIN_KB`, default `1024` KiB = 1 MiB).
///
/// Under `VICTORY_CARD_BUFFERS_MIN_KB=64`, ~4,045 small (`< 1 MiB`) buffers in the
/// title's `0x2404` write-combining heap become card copies. Skipping compare/copy-back
/// on those small WC buffers drops ~263 pieces/frame and ~744 draws/frame and causes
/// ~70% black frames (NaN/exposure feedback), whereas skipping `>= 1 MiB` WC buffers
/// saves ~39 MB/frame (516 pieces/frame) with 0/230 black frames.
pub(crate) fn skip_wc_min_bytes() -> u64 {
    static MIN: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *MIN.get_or_init(|| {
        let kib = std::env::var("VICTORY_SKIP_WC_MIN_KB")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(1024);
        kib << 10
    })
}

/// `VICTORY_SKIP_WC_BACK=1`: what shaders wrote to a card copy of title memory
/// reserved write-combining (`protect & 0x400 != 0`) is not fetched back into
/// title memory (opt-in; off by default).
pub(crate) fn skip_wc_back() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        let on = skip_wc_compare() || std::env::var_os("VICTORY_SKIP_WC_BACK").is_some_and(|v| v == "1");
        if on {
            tracing::info!(
                min_kb = skip_wc_min_bytes() >> 10,
                "VICTORY_SKIP_WC_BACK=1: shader writes to write-combining title memory >= min_kb stay on the card"
            );
        }
        on
    })
}

/// `VICTORY_SKIP_WC_COMPARE=1` (implies `VICTORY_SKIP_WC_BACK=1`, opt-in experiment):
/// also skips card-side compare passes on write-combining title memory `>= VICTORY_SKIP_WC_MIN_KB`.
pub(crate) fn skip_wc_compare() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        let on = std::env::var_os("VICTORY_SKIP_WC_COMPARE").is_some_and(|v| v == "1");
        if on {
            tracing::warn!(
                min_kb = skip_wc_min_bytes() >> 10,
                "VICTORY_SKIP_WC_COMPARE=1: write-combining card copies >= min_kb skip compare passes (experiment)"
            );
        }
        on
    })
}

/// Every declared title range must lie wholly inside a single WC reservation.
pub(crate) fn all_wc(ranges: &[(usize, usize)]) -> bool {
    let reservations = RESERVATIONS.lock().unwrap_or_else(|e| e.into_inner());
    all_wc_in(&reservations, ranges)
}

fn all_wc_in(reservations: &BTreeMap<usize, Reservation>, ranges: &[(usize, usize)]) -> bool {
    !ranges.is_empty()
        && ranges.iter().all(|&(from, to)| {
            from < to
                && reservations.range(..to).next_back().is_some_and(|(&base, r)| {
                    base <= from && r.protect & 0x400 != 0 && base.checked_add(r.len).is_some_and(|end| to <= end)
                })
        })
}

pub(crate) static UNCOMPARED: AtomicU64 = AtomicU64::new(0);
pub(crate) static SKIPPED: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];

pub(crate) fn skip_back(copy_bytes: u64, declared: (usize, usize), bytes: u64) -> bool {
    if !skip_wc_back() || copy_bytes < skip_wc_min_bytes() {
        return false;
    }
    let wc = all_wc(&[declared]);
    if wc {
        let n = SKIPPED[0].fetch_add(1, Ordering::Relaxed) + 1;
        let b = SKIPPED[1].fetch_add(bytes, Ordering::Relaxed) + bytes;
        if n.is_power_of_two() {
            tracing::info!(
                "VICTORY_SKIP_WC_BACK: {n} pieces ({} MB) of write-combining memory left on the card so far",
                b >> 20
            );
        }
    }
    wc
}

#[cfg(test)]
mod tests {
    use super::{Reservation, all_wc_in, skip_wc_min_bytes};
    use std::collections::BTreeMap;

    fn res(len: usize, protect: u32) -> Reservation {
        Reservation { len, _kind: 0, _flags: 0, protect }
    }

    #[test]
    fn wc_reservation_boundaries_hold() {
        const MIB: usize = 1 << 20;
        let window = 8 * MIB;
        let middle = window + 2 * MIB;
        let map = BTreeMap::from([(window, res(2 * MIB, 0x404)), (middle, res(2 * MIB, 0x04))]);
        assert!(all_wc_in(&map, &[(middle - 65536, middle)]));
        assert!(!all_wc_in(&map, &[(middle, middle + 65536)]));
        assert!(!all_wc_in(&map, &[(middle - 65536, middle + 65536)]));
    }

    #[test]
    fn default_skip_wc_min_bytes_protects_sub_1mb_buffers() {
        assert_eq!(skip_wc_min_bytes(), 1 << 20);
        assert!((64 << 10) < skip_wc_min_bytes());
        assert!((1 << 20) >= skip_wc_min_bytes());
    }
}
