//! DLSS beside the title's own FidelityFX DLLs (`VICTORY_DLSS_DIR`).
//!
//! The executable's five FidelityFX imports from
//! `amd_fidelityfx_loader_xbox.dll` are bound here ([`take`], [`ours`]); the
//! DLLs themselves stay the title's and are called for everything: the
//! context is the title's FSR context, made, configured, queried and
//! destroyed by its own code, and a DLSS context (a Streamline viewport) is
//! made beside it. An upscale dispatch is evaluated by DLSS on the host
//! GPU; one that DLSS does not evaluate (no DLSS context, a texture without
//! its host copy, Streamline failing) goes to the title's own `ffxDispatch`
//! when that can take it. The first dispatches, and every reason one is
//! not evaluated, are logged.
//!
//! With the title's list calls written down (`VICTORY_THIN=1`) its list
//! has no host list at the call: the evaluation is written down in the
//! list's stream instead (`crate::list::thin::defer`) and done by the
//! thread that translates the stream, at that point, on its list of ours
//! ([`Deferred`]). FSR cannot stand in then (its own dispatch would have
//! had to be recorded at the call), so a dispatch that cannot be evaluated
//! there is said, and that frame not upscaled.
//!
//! The title's FSR4 is a fixed-resolution build: of maximum render sizes it
//! takes only 2/3 of the output ("tight", 1280x720 for 1920x1080) or the
//! output itself ("legacy"), and of per-frame render sizes only the tight
//! one; a context it refuses leaves the title dividing by zero. So its
//! context is always made with a maximum it takes ([`fsr_max`]), the DLSS
//! context with the size `VICTORY_DLSS_MODE` renders at, and a frame below
//! the tight size that DLSS did not upscale is not given to FSR (it is
//! said, and the dispatch fails).
#![allow(non_snake_case)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use super::{
    CREATE_UPSCALE, Configure, Create, CreateUpscale, DISPATCH_UPSCALE, Destroy, Dispatch,
    DispatchUpscale, ERROR, Header, OK, OUTPUT, Query, ReturnCode, dlss, host_device,
    record_on_host, upscale_in,
};

/// The DLL whose exports are taken.
pub const LOADER: &str = "amd_fidelityfx_loader_xbox.dll";

/// The calls taken, by export name.
pub const NAMES: [&str; 5] = [
    "ffxCreateContext",
    "ffxDestroyContext",
    "ffxConfigure",
    "ffxQuery",
    "ffxDispatch",
];

/// How many dispatches are logged one by one.
const LOGGED: u32 = 8;

/// The title's own FidelityFX calls.
struct Real {
    create: Create,
    destroy: Destroy,
    configure: Configure,
    query: Query,
    dispatch: Dispatch,
}

static REAL: OnceLock<Real> = OnceLock::new();

/// A title context (its address), the DLSS context made beside it (0: none)
/// and the one per-frame render size its FSR takes.
#[derive(Clone, Copy)]
struct Entry {
    title: usize,
    ours: usize,
    fsr_render: [u32; 2],
}

static CONTEXTS: Mutex<Vec<Entry>> = Mutex::new(Vec::new());

/// Takes the title's own FidelityFX calls, by export name (`export`, the
/// title's loader DLL): true when all five are there. Without them nothing
/// is taken and FSR stays the title's.
pub fn take(export: &dyn Fn(&str) -> Option<usize>) -> bool {
    let [Some(c), Some(d), Some(g), Some(q), Some(x)] = NAMES.map(export) else {
        tracing::warn!("dlss: the title's FidelityFX loader lacks an export; DLSS off, FSR stays");
        return false;
    };
    // SAFETY: the title's exports of these names, of these types
    // (`ffx_api.h`, the console's ABI is the host's).
    let real = unsafe {
        Real {
            create: std::mem::transmute::<usize, Create>(c),
            destroy: std::mem::transmute::<usize, Destroy>(d),
            configure: std::mem::transmute::<usize, Configure>(g),
            query: std::mem::transmute::<usize, Query>(q),
            dispatch: std::mem::transmute::<usize, Dispatch>(x),
        }
    };
    REAL.set(real).is_ok() || REAL.get().is_some()
}

/// What the executable's import `name` of [`LOADER`] is bound to, once
/// [`take`] took the title's calls.
pub fn ours(name: &str) -> Option<usize> {
    REAL.get()?;
    Some(match name {
        "ffxCreateContext" => ffxCreateContext as *const () as usize,
        "ffxDestroyContext" => ffxDestroyContext as *const () as usize,
        "ffxConfigure" => ffxConfigure as *const () as usize,
        "ffxQuery" => ffxQuery as *const () as usize,
        "ffxDispatch" => ffxDispatch as *const () as usize,
        _ => return None,
    })
}

/// The tight size of the title's FSR4 for an output: 2/3 of it.
fn tight(upscale: [u32; 2]) -> [u32; 2] {
    upscale.map(|v| v * 2 / 3)
}

/// The maximum render size the title's FSR4 is given: `max_render` when it
/// takes it (the tight size or the output's), the tight size otherwise.
fn fsr_max(max_render: [u32; 2], max_upscale: [u32; 2]) -> [u32; 2] {
    if max_render == tight(max_upscale) || max_render == max_upscale {
        max_render
    } else {
        tight(max_upscale)
    }
}

/// The DLSS context's maximum render size: the size `VICTORY_DLSS_MODE`
/// renders at (`scale` of the output), the title's otherwise.
fn dlss_max(max_render: [u32; 2], max_upscale: [u32; 2], scale: Option<f32>) -> [u32; 2] {
    match scale {
        Some(s) => max_upscale.map(|v| (v as f32 * s).round() as u32),
        None => max_render,
    }
}

fn contexts() -> std::sync::MutexGuard<'static, Vec<Entry>> {
    CONTEXTS.lock().unwrap_or_else(|e| e.into_inner())
}

/// The entry of the title's context `context` points at.
fn entry(context: *mut *mut c_void) -> Option<Entry> {
    // SAFETY: the title's context pointer, checked for null.
    let title = (!context.is_null()).then(|| unsafe { *context } as usize)?;
    contexts()
        .iter()
        .find(|e| e.title == title && title != 0)
        .copied()
}

/// The upscale description in the title's chain, to change in place.
///
/// # Safety
///
/// `h` is null or the start of a chain of valid descriptions.
unsafe fn upscale_at(mut h: *mut Header) -> Option<*mut CreateUpscale> {
    while !h.is_null() {
        // SAFETY: the caller's chain.
        if unsafe { (*h).kind } == CREATE_UPSCALE {
            return Some(h.cast());
        }
        // SAFETY: as above.
        h = unsafe { (*h).next };
    }
    None
}

extern "C" fn ffxCreateContext(
    context: *mut *mut c_void,
    desc: *mut Header,
    memory: *const c_void,
) -> ReturnCode {
    let Some(real) = REAL.get() else { return ERROR };
    // SAFETY: the title's chain of descriptions (null checked).
    let upscale: Option<CreateUpscale> = (!desc.is_null())
        .then(|| unsafe { upscale_in(desc) })
        .flatten();
    // The title's FSR gets a maximum it takes, for the call only.
    // SAFETY: as above.
    let at = (!desc.is_null()).then(|| unsafe { upscale_at(desc) }).flatten();
    let mut restore = None;
    if let (Some(u), Some(at)) = (upscale, at) {
        let max = fsr_max(u.max_render, u.max_upscale);
        if max != u.max_render {
            tracing::warn!(
                asked = ?u.max_render,
                given = ?max,
                upscale = ?u.max_upscale,
                "dlss: the title's FSR4 takes no such maximum render size; its context gets the tight one, DLSS renders at the asked one"
            );
            // SAFETY: the title's description, restored below.
            unsafe { (*at).max_render = max };
            restore = Some(u.max_render);
        }
    }
    // SAFETY: the title's own call with its own arguments.
    let r = unsafe { (real.create)(context, desc, memory) };
    if let (Some(old), Some(at)) = (restore, at) {
        // SAFETY: as the title made it.
        unsafe { (*at).max_render = old };
    }
    // SAFETY: the title's context pointer, checked for null.
    if r != OK || context.is_null() || unsafe { (*context).is_null() } {
        tracing::error!(result = r, "dlss: the title's FSR refused its upscale context");
        return r;
    }
    // SAFETY: checked non-null above.
    let title = unsafe { *context } as usize;
    let Some(mut upscale) = upscale else {
        tracing::warn!("dlss: a FidelityFX context without an upscale description; FSR upscales it");
        return r;
    };
    let mut entry = Entry {
        title,
        ours: 0,
        fsr_render: tight(upscale.max_upscale),
    };
    let scale = dlss::requested().render_scale();
    upscale.max_render = dlss_max(upscale.max_render, upscale.max_upscale, scale);
    let mut slot = std::ptr::null_mut();
    match host_device() {
        Some(device) if dlss::create(&mut slot, device, &upscale) == OK && !slot.is_null() => {
            entry.ours = slot as usize;
        }
        _ => {
            tracing::warn!(
                render = ?upscale.max_render,
                fsr = ?entry.fsr_render,
                "dlss: no DLSS context; the title's FSR upscales instead, at its own size only (frames rendered smaller go without; use VICTORY_DLSS_MODE=quality)"
            );
        }
    }
    contexts().push(entry);
    r
}

extern "C" fn ffxDestroyContext(context: *mut *mut c_void, memory: *const c_void) -> ReturnCode {
    let Some(real) = REAL.get() else { return ERROR };
    if let Some(e) = entry(context) {
        contexts().retain(|x| x.title != e.title);
        if e.ours != 0 {
            let mut slot = e.ours as *mut c_void;
            dlss::destroy(&mut slot);
        }
    }
    // SAFETY: the title's own call with its own arguments.
    unsafe { (real.destroy)(context, memory) }
}

extern "C" fn ffxConfigure(context: *mut *mut c_void, desc: *const Header) -> ReturnCode {
    let Some(real) = REAL.get() else { return ERROR };
    // SAFETY: the title's own call with its own arguments.
    unsafe { (real.configure)(context, desc) }
}

extern "C" fn ffxQuery(context: *mut *mut c_void, desc: *mut Header) -> ReturnCode {
    match REAL.get() {
        // SAFETY: the title's own call with its own arguments.
        Some(real) => unsafe { (real.query)(context, desc) },
        // SAFETY: the title's description, if any.
        None if !desc.is_null() => unsafe { dlss::query(desc) }.unwrap_or(ERROR),
        None => ERROR,
    }
}

/// The textures a DLSS evaluation needs: colour, depth, motion, output.
const NEEDED: [usize; 4] = [0, 1, 2, OUTPUT];

/// A DLSS evaluation written down in a title list's stream: the title's
/// description as it was at the call (its objects live until the list has
/// run) and our DLSS context.
struct Deferred {
    d: DispatchUpscale,
    ours: usize,
    /// The title's frame, for Streamline, with frame generation on
    /// (`super::framegen::frame_index`).
    frame: Option<u32>,
}

// SAFETY: plain data and the title's objects, used by one thread at a time.
unsafe impl Send for Deferred {}

impl Deferred {
    /// Evaluates on `back`, the list of ours the stream is translated on.
    fn run(mut self, back: crate::object::This) {
        static COUNT: AtomicU32 = AtomicU32::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        self.d.command_list = back.cast_mut().cast();
        let mut slot = self.ours as *mut c_void;
        // SAFETY: a description of ours; DLSS records on the host list
        // behind our list, at this point of the title's.
        let r = unsafe {
            record_on_host((&raw const self.d).cast(), &NEEDED, |d| {
                dlss::dispatch(&mut slot, d, self.frame)
            })
        };
        match r {
            Ok(OK) => {
                if n <= LOGGED {
                    tracing::info!(n, "dlss: upscale dispatch evaluated by DLSS (where the list was translated)");
                }
            }
            Ok(_) => skipped(n, "DLSS failed (see the dlss: error above)", Some((self.d.render_size, [0, 0]))),
            Err(why) => skipped(n, why, Some((self.d.render_size, [0, 0]))),
        }
    }
}

/// Writes the evaluation down in the title list's stream when its calls
/// are written down (`VICTORY_THIN=1`); false when they are not, or there
/// is nothing DLSS could evaluate (no DLSS context, a texture missing).
fn written_down(e: Option<Entry>, desc: *const Header, n: u32) -> bool {
    let Some(Entry { ours, .. }) = e.filter(|e| e.ours != 0) else {
        return false;
    };
    // SAFETY: the title's description, read by its type (null checked).
    if desc.is_null() || unsafe { (*desc).kind } != DISPATCH_UPSCALE {
        return false;
    }
    // SAFETY: as above.
    let d = unsafe { *desc.cast::<DispatchUpscale>() };
    if NEEDED.iter().any(|&i| d.resources[i].resource.is_null()) {
        return false;
    }
    let list = d.command_list as crate::object::This;
    let job = Deferred { d, ours, frame: super::framegen::frame_index() };
    if crate::list::thin::defer(list, Box::new(move |back| job.run(back))).is_err() {
        return false;
    }
    if n <= LOGGED {
        tracing::info!(n, "dlss: upscale dispatch written down; DLSS evaluates it where the list is translated");
    }
    true
}

extern "C" fn ffxDispatch(context: *mut *mut c_void, desc: *const Header) -> ReturnCode {
    let Some(real) = REAL.get() else { return ERROR };
    static COUNT: AtomicU32 = AtomicU32::new(0);
    let n = COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    // Frame generation's depth, motion vectors and numbers, whoever
    // upscales (`super::framegen`).
    // SAFETY: the title's description (null checked there).
    unsafe { super::framegen::from_ffx(desc) };
    let e = entry(context);
    if written_down(e, desc, n) {
        return OK;
    }
    let evaluated = match e {
        _ if desc.is_null() => Err("no description"),
        None => Err("not a context made through DLSS's bridge"),
        Some(Entry { ours: 0, .. }) => Err("no DLSS context for this FidelityFX context"),
        Some(Entry { ours, .. }) => {
            let mut slot = ours as *mut c_void;
            // SAFETY: the title's non-null description; DLSS records on
            // the host list the swap gives it.
            unsafe {
                record_on_host(desc, &NEEDED, |d| {
                    dlss::dispatch(&mut slot, d, super::framegen::frame_index())
                })
            }
            .and_then(|r| {
                if r == OK {
                    Ok(())
                } else {
                    Err("DLSS failed (see the dlss: error above)")
                }
            })
        }
    };
    if evaluated.is_ok() {
        if n <= LOGGED {
            tracing::info!(n, "dlss: upscale dispatch evaluated by DLSS");
        }
        return OK;
    }
    let why = evaluated.err().unwrap_or_default();
    // SAFETY: the title's non-null description, read by its type.
    let render = (!desc.is_null() && unsafe { (*desc).kind } == DISPATCH_UPSCALE)
        .then(|| unsafe { (*desc.cast::<DispatchUpscale>()).render_size });
    if let (Some(e), Some(render)) = (e, render)
        && render != e.fsr_render
    {
        skipped(n, why, Some((render, e.fsr_render)));
        return ERROR;
    }
    skipped(n, why, None);
    // SAFETY: the title's own call with its own arguments.
    unsafe { (real.dispatch)(context, desc) }
}

/// Says that dispatch `n` was not evaluated by DLSS, and whether FSR got it
/// (`refused`: the render size it was rendered at and the one FSR takes,
/// so not): each of the first ones, then each reason once.
fn skipped(n: u32, why: &'static str, refused: Option<([u32; 2], [u32; 2])>) {
    static SEEN: Mutex<Vec<(&str, bool)>> = Mutex::new(Vec::new());
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let key = (why, refused.is_some());
    let new = !seen.contains(&key);
    if new {
        seen.push(key);
    }
    if n > LOGGED && !new {
        return;
    }
    match refused {
        Some((render, [0, 0])) => tracing::error!(
            n,
            why,
            ?render,
            "dlss: upscale dispatch written down but not evaluated where the list was translated: frame not upscaled"
        ),
        Some((render, fsr)) => tracing::error!(
            n,
            why,
            ?render,
            ?fsr,
            "dlss: upscale dispatch not evaluated, and the title's FSR4 takes only its own render size: frame not upscaled"
        ),
        None => tracing::warn!(n, why, "dlss: upscale dispatch not evaluated; the title's FSR upscales it"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_taken_without_every_export() {
        let partial = |name: &str| (name != "ffxQuery").then_some(0x1000);
        assert!(!take(&partial));
        assert!(ours("ffxDispatch").is_none());
        assert!(entry(std::ptr::null_mut()).is_none());
    }

    #[test]
    fn the_title_fsr_gets_a_maximum_it_takes_and_dlss_the_mode_size() {
        let out = [1920, 1080];
        // The tight and the legacy sizes stay; anything else is the tight.
        assert_eq!(fsr_max([1280, 720], out), [1280, 720]);
        assert_eq!(fsr_max([1920, 1080], out), [1920, 1080]);
        assert_eq!(fsr_max([960, 540], out), [1280, 720]);
        // DLSS renders at its mode's size, or the title's.
        assert_eq!(dlss_max([1280, 720], out, Some(0.5)), [960, 540]);
        assert_eq!(dlss_max([1280, 720], out, Some(1.0 / 3.0)), [640, 360]);
        assert_eq!(dlss_max([1280, 720], out, None), [1280, 720]);
    }
}
