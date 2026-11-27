//! `VICTORY_PARTIAL_DELTA=1`, default off: an upload of a texture picked by
//! index into a host copy that was uploaded before copies only the
//! subresources (level x array slice) whose title bytes differ from what this
//! host copy got from its last submitted upload (credit `@zocram4cc`, `096e9f0c`).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{LazyLock, Mutex};

use super::Digest;

pub(crate) fn on() -> bool {
    static ON: LazyLock<bool> = LazyLock::new(|| crate::switch::on("VICTORY_PARTIAL_DELTA"));
    *ON
}

/// Per host copy serial: subresource key (level << 16 | slice) -> digest of
/// the title bytes its last submitted upload was detiled from.
static RECORD: Mutex<Option<HashMap<u64, HashMap<u32, Digest>>>> = Mutex::new(None);

/// Most host copies with a record; past it the record starts over.
const MOST: usize = 65_536;

/// Since the last line: plans, subresources copied, left out, bytes left out,
/// left out but changed by the submission, submissions that read nothing for
/// a plan with some left out, records forgotten.
static N: [AtomicU64; 7] = [const { AtomicU64::new(0) }; 7];
static LINE_MS: AtomicU64 = AtomicU64::new(0);

/// One list's upload into one host copy.
pub(crate) struct Plan {
    serial: u64,
    parts: Vec<((usize, usize), u32, Option<Digest>)>,
    verified: bool,
    stale: bool,
}

impl Plan {
    pub(crate) fn skips(&self, i: usize) -> bool {
        self.parts.get(i).is_some_and(|p| p.2.is_some())
    }

    pub(crate) fn any_skipped(&self) -> bool {
        self.parts.iter().any(|p| p.2.is_some())
    }
}

/// The title span of every subresource, or None where the layout is not one
/// whose subresources are read from spans of their own.
pub(crate) fn spans(
    layout: &victory_formats::texture::Layout,
    parts: impl Iterator<Item = (u32, u32)>,
    read: usize,
) -> Option<Vec<((usize, usize), u32)>> {
    use victory_formats::texture::Dimension;
    if layout.desc.dimension != Dimension::Tex2d || layout.thick || layout.desc.samples != 1 {
        return None;
    }
    let element = u64::from(layout.desc.element_bytes);
    let mut out = Vec::new();
    for (mip, slice) in parts {
        let m = layout.mips.get(mip as usize)?;
        if m.depth != 1 || slice >= 1 << 16 || mip >= 1 << 16 {
            return None;
        }
        let base = u64::from(slice) * layout.slice_size;
        let (start, len) = match m.tail {
            Some(_) => (base + m.offset, layout.alignment),
            None => (base + m.offset, u64::from(m.pitch) * u64::from(m.rows) * element),
        };
        let end = start.checked_add(len)?;
        if len == 0 || end > read as u64 {
            return None;
        }
        out.push(((start as usize, len as usize), mip << 16 | slice));
    }
    Some(out)
}

thread_local! {
    static BUF: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn digest_now(memory: *const u8, (at, len): (usize, usize)) -> Digest {
    BUF.with(|b| {
        let mut b = b.borrow_mut();
        if b.len() < len {
            b.resize(len, 0);
        }
        let buf = &mut b[..len];
        buf.fill(0);
        // SAFETY: inside the texture's span (`spans` checked `read`).
        crate::host::copy_mapped(buf.as_mut_ptr(), unsafe { memory.add(at) }, len, false);
        super::digest(buf)
    })
}

pub(crate) fn plan(serial: u64, memory: *const u8, spans: Vec<((usize, usize), u32)>) -> Plan {
    let mut record = RECORD.lock().unwrap_or_else(|e| e.into_inner());
    let record = record.get_or_insert_with(HashMap::new);
    let mut parts = Vec::with_capacity(spans.len());
    match record.get_mut(&serial) {
        Some(known) => {
            for (span, key) in spans {
                let left_out = match known.get(&key) {
                    Some(&was) if digest_now(memory, span) == was => Some(was),
                    Some(_) => {
                        known.remove(&key);
                        None
                    }
                    None => None,
                };
                parts.push((span, key, left_out));
            }
        }
        None => parts.extend(spans.into_iter().map(|(span, key)| (span, key, None))),
    }
    N[0].fetch_add(1, Relaxed);
    Plan { serial, parts, verified: false, stale: false }
}

pub(crate) fn detiled(plan: &mut Plan, i: usize, tiled: &[u8], bytes: u64) -> bool {
    let Some(&((at, len), key, left_out)) = plan.parts.get(i) else {
        return true;
    };
    plan.verified = true;
    let Some(bytes_now) = tiled.get(at..at + len) else {
        plan.stale |= left_out.is_some();
        return true;
    };
    let now = super::digest(bytes_now);
    match left_out {
        Some(was) => {
            if was != now {
                plan.stale = true;
                N[4].fetch_add(1, Relaxed);
            } else {
                N[2].fetch_add(1, Relaxed);
                N[3].fetch_add(bytes, Relaxed);
            }
            false
        }
        None => {
            N[1].fetch_add(1, Relaxed);
            let mut record = RECORD.lock().unwrap_or_else(|e| e.into_inner());
            let record = record.get_or_insert_with(HashMap::new);
            if record.len() >= MOST && !record.contains_key(&plan.serial) {
                record.clear();
            }
            record.entry(plan.serial).or_default().insert(key, now);
            true
        }
    }
}

pub(crate) fn settle(plan: &mut Plan, start: u64) -> bool {
    if plan.verified && !plan.stale {
        return true;
    }
    if !plan.any_skipped() {
        if !plan.verified {
            forget(plan.serial);
        }
        return true;
    }
    if !plan.verified {
        N[5].fetch_add(1, Relaxed);
    }
    forget(plan.serial);
    crate::descriptor::host::levels_waiting(start);
    false
}

pub(crate) fn forget(serial: u64) {
    let mut record = RECORD.lock().unwrap_or_else(|e| e.into_inner());
    if record.as_mut().and_then(|r| r.remove(&serial)).is_some() {
        N[6].fetch_add(1, Relaxed);
    }
}

pub(crate) fn line() {
    let now = crate::host::now_ms();
    let last = LINE_MS.load(Relaxed);
    if now < last + 5000 || LINE_MS.compare_exchange(last, now, Relaxed, Relaxed).is_err() {
        return;
    }
    let n: Vec<u64> = N.iter().map(|n| n.swap(0, Relaxed)).collect();
    let copies = RECORD.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map_or(0, HashMap::len);
    tracing::info!(
        "partial delta (VICTORY_PARTIAL_DELTA=1), last 5 s: {} uploads planned, subresources copied {}, left out {} ({} MB not copied), \
         left out but changed by the submission {} (made due again), plans the submission did not read {}, records forgotten {}; host copies with a record {copies}",
        n[0],
        n[1],
        n[2],
        n[3] >> 20,
        n[4],
        n[5],
        n[6]
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use victory_formats::texture::{Desc, Dimension, Layout, Swizzle};

    fn descs() -> Vec<Desc> {
        let mut out = Vec::new();
        for swizzle in [Swizzle::LINEAR, Swizzle::SW_256B_S, Swizzle::SW_4KB_S, Swizzle::SW_4KB_D, Swizzle::SW_64KB_S, Swizzle::SW_64KB_D] {
            for (width, height, mips, element_bytes, block, depth) in [
                (512, 512, 10, 8, 4, 1),
                (1024, 256, 11, 16, 4, 1),
                (256, 256, 9, 4, 1, 1),
                (300, 200, 9, 2, 1, 1),
                (128, 128, 8, 8, 4, 6),
                (2048, 2048, 4, 16, 4, 1),
            ] {
                out.push(Desc { dimension: Dimension::Tex2d, width, height, depth, mips, element_bytes, block, samples: 1, swizzle });
            }
        }
        out
    }

    #[test]
    fn a_subresource_reads_only_its_span() {
        let mut tried = 0;
        for desc in descs() {
            let Ok(layout) = Layout::new(&desc) else { continue };
            let read = layout.size as usize;
            let parts: Vec<(u32, u32)> =
                (0..layout.slices).flat_map(|s| (0..layout.mips.len() as u32).map(move |m| (m, s))).collect();
            let Some(spans) = super::spans(&layout, parts.iter().copied(), read) else { continue };
            let tiled: Vec<u8> = (0..read).map(|i| (i * 131 + i / 7) as u8).collect();
            for (&(mip, slice), &((at, len), _)) in parts.iter().zip(&spans) {
                let m = layout.mips[mip as usize];
                let pitch = (m.width as usize).max(1) * 16;
                let rows = m.height as usize;
                let mut a = vec![0u8; pitch * rows.max(1)];
                let mut b = a.clone();
                let mut other = vec![0xa5u8; read];
                other[at..at + len].copy_from_slice(&tiled[at..at + len]);
                if layout.detile(&tiled, mip, slice, &mut a, pitch, pitch * rows.max(1)).is_err() {
                    continue;
                }
                layout.detile(&other, mip, slice, &mut b, pitch, pitch * rows.max(1)).unwrap();
                assert!(a == b, "{desc:?} mip {mip} slice {slice}: the detiling read outside {at:#x}+{len:#x}");
                tried += 1;
            }
        }
        assert!(tried > 0, "no layout tried");
    }

    fn three_parts() -> (Vec<u8>, Vec<((usize, usize), u32)>) {
        let memory: Vec<u8> = (0..512u32).map(|i| (i * 7 + 3) as u8).collect();
        (memory, vec![((0, 64), 0), ((128, 64), 1 << 16), ((256, 64), 2 << 16)])
    }

    fn detile_all(p: &mut Plan, memory: &[u8]) -> usize {
        (0..3).filter(|&i| detiled(p, i, memory, 64)).count()
    }

    #[test]
    fn unchanged_parts_are_left_out_of_the_next_copy() {
        let (mut memory, spans) = three_parts();
        let start = memory.as_ptr();
        let mut first = plan(7, start, spans.clone());
        assert!(!first.any_skipped());
        assert_eq!(detile_all(&mut first, &memory), 3);
        assert!(settle(&mut first, start as u64));
        let second = plan(7, start, spans.clone());
        assert!((0..3).all(|i| second.skips(i)));
        memory[128] ^= 0xff;
        let third = plan(7, start, spans.clone());
        assert!(third.skips(0) && !third.skips(1) && third.skips(2));
    }

    #[test]
    fn a_left_out_part_changed_before_the_submission_makes_the_upload_stale() {
        let (mut memory, spans) = three_parts();
        let start = memory.as_ptr();
        let mut first = plan(9, start, spans.clone());
        detile_all(&mut first, &memory);
        assert!(settle(&mut first, start as u64));
        let mut second = plan(9, start, spans.clone());
        assert!(second.skips(0));
        memory[0] ^= 1;
        assert!(!detiled(&mut second, 0, &memory, 64));
        assert!(!settle(&mut second, start as u64));
        assert!(!plan(9, start, spans.clone()).any_skipped());
    }

    #[test]
    fn a_host_copy_written_by_the_card_has_no_record() {
        let (memory, spans) = three_parts();
        let start = memory.as_ptr();
        let mut first = plan(11, start, spans.clone());
        detile_all(&mut first, &memory);
        assert!(settle(&mut first, start as u64));
        assert!(plan(11, start, spans.clone()).any_skipped());
        forget(11);
        assert!(!plan(11, start, spans.clone()).any_skipped());
    }
}
