//! Room for our own code beside the title's: the stubs the site patchers
//! (`write_sites`, `descriptor_sites`) put in place of an instruction are
//! reached by a 5-byte `jmp rel32` written over it, and jump back the same
//! way, so they must lie within a signed 32-bit displacement of the code
//! they patch.

use windows_sys::Win32::System::Memory::{
    MEM_COMMIT, MEM_FREE, MEM_RESERVE, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE_READWRITE, VirtualAlloc, VirtualQuery,
};

/// The bytes of one block [`alloc_near`] gives.
pub(crate) const LEN: usize = 64 << 10;

/// Windows places allocations on 64 KiB boundaries.
const GRANULE: usize = 0x1_0000;

/// How far from the patched code a block may start: with the block's own
/// length added this stays well inside the 2 GiB a `rel32` reaches either
/// way.
const REACH: usize = 0x7000_0000;

/// `jmp rel32` (`e9 cd`) placed at `from`, landing on `to`; `None` when the
/// displacement does not fit 32 bits.
pub(crate) fn jmp_rel32(from: usize, to: usize) -> Option<[u8; 5]> {
    // The displacement counts from the instruction after the jump.
    let next = i128::try_from(from).ok()? + 5;
    let delta = i32::try_from(i128::try_from(to).ok()? - next).ok()?;
    let mut code = [0xe9, 0, 0, 0, 0];
    code[1..].copy_from_slice(&delta.to_le_bytes());
    Some(code)
}

/// The places inside free address space between `low` and `high` where a
/// block of [`LEN`] bytes fits, nearest `code` first. Pure, for testing:
/// `free` lists the free regions as (start, size).
fn candidates(free: &[(usize, usize)], code: usize, low: usize, high: usize) -> Vec<usize> {
    let mut found = Vec::new();
    for &(start, size) in free {
        let end = start.saturating_add(size).min(high);
        let first = start.max(low).next_multiple_of(GRANULE);
        if first.saturating_add(LEN) > end {
            continue;
        }
        let last = (end - LEN) & !(GRANULE - 1);
        // The aligned place of this region closest to the code.
        let best = (code & !(GRANULE - 1)).clamp(first, last);
        found.push(best);
    }
    found.sort_by_key(|&at| at.abs_diff(code));
    found
}

/// The free regions of this process's address space between `low` and
/// `high`, as (start, size).
fn free_regions(low: usize, high: usize) -> Vec<(usize, usize)> {
    let mut regions = Vec::new();
    let mut at = low;
    while at < high {
        // SAFETY: zeroed is a valid MEMORY_BASIC_INFORMATION, filled in by
        // the call; any address may be asked about.
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: as above, with the size of the structure passed.
        let got = unsafe { VirtualQuery(at as *const _, &mut info, size_of::<MEMORY_BASIC_INFORMATION>()) };
        if got == 0 || info.RegionSize == 0 {
            break;
        }
        let start = info.BaseAddress as usize;
        if info.State == MEM_FREE {
            regions.push((start, info.RegionSize));
        }
        let Some(next) = start.checked_add(info.RegionSize) else {
            break;
        };
        if next <= at {
            break;
        }
        at = next;
    }
    regions
}

/// A block of [`LEN`] bytes of committed, executable and writable memory
/// whose every byte a `jmp rel32` at `code` reaches, and that reaches back:
/// the free place nearest to it.
pub(crate) fn alloc_near(code: usize) -> Option<usize> {
    let low = code.saturating_sub(REACH).max(GRANULE);
    let high = code.saturating_add(REACH);
    for place in candidates(&free_regions(low, high), code, low, high) {
        // SAFETY: asks for free address space at `place`; fails when another
        // thread took it meanwhile.
        let got = unsafe {
            VirtualAlloc(place as *const _, LEN, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE)
        } as usize;
        if got != 0 {
            return Some(got);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jumps_count_from_the_next_instruction() {
        assert_eq!(jmp_rel32(0x1000, 0x1005), Some([0xe9, 0, 0, 0, 0]));
        assert_eq!(jmp_rel32(0x1000, 0x1105), Some([0xe9, 0x00, 0x01, 0, 0]));
        // Backwards: -5 is a jump to itself.
        assert_eq!(jmp_rel32(0x1000, 0x1000), Some([0xe9, 0xfb, 0xff, 0xff, 0xff]));
        let far = 0x7fff_0000_0000usize;
        assert_eq!(jmp_rel32(far, far + 5 + i32::MAX as usize), Some([0xe9, 0xff, 0xff, 0xff, 0x7f]));
        assert_eq!(jmp_rel32(far, far + 6 + i32::MAX as usize), None, "one byte too far");
        assert_eq!(jmp_rel32(far, far + 5 - (1usize << 31)), Some([0xe9, 0, 0, 0, 0x80]));
        assert_eq!(jmp_rel32(far, far + 4 - (1usize << 31)), None);
    }

    #[test]
    fn places_are_aligned_inside_free_space_and_nearest_first() {
        let code = 0x1_4000_1234;
        let free = [
            // too small for a block
            (0x1_3000_0000, 0x8000),
            // below the code
            (0x1_2000_0000, 0x10_0000),
            // around the code itself (as if it were free)
            (0x1_3fff_0000, 0x4_0000),
            // above, starting off a boundary
            (0x1_5000_1000, 0x20_0000),
        ];
        let got = candidates(&free, code, GRANULE, usize::MAX);
        assert_eq!(got, [0x1_4000_0000, 0x1_5001_0000, 0x1_200f_0000]);
        for at in got {
            assert_eq!(at % GRANULE, 0);
        }
        // Limits cut regions too.
        assert_eq!(candidates(&free, code, 0x1_4000_0000, 0x1_4002_0000), [0x1_4000_0000]);
        assert!(candidates(&free, code, 0x1_6000_0000, usize::MAX).is_empty());
    }

    #[test]
    fn a_block_near_our_own_code_is_reachable_and_writable() {
        let code = a_block_near_our_own_code_is_reachable_and_writable as *const () as usize;
        let block = alloc_near(code).expect("free space near the loader's code");
        for at in [block, block + LEN - 5] {
            assert!(jmp_rel32(code, at).is_some());
            assert!(jmp_rel32(at, code).is_some());
        }
        // SAFETY: the block was just committed read-write-execute for us.
        unsafe {
            (block as *mut u8).write_bytes(0xcc, LEN);
            assert_eq!(*((block + LEN - 1) as *const u8), 0xcc);
        }
    }
}
