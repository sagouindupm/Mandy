//! DLSS beside the title's own FidelityFX DLLs (`VICTORY_DLSS_DIR`).
//!
//! The executable's five FidelityFX imports from
//! `amd_fidelityfx_loader_xbox.dll` are bound here ([`take`], [`ours`]); the
//! DLLs themselves stay the title's and are called for everything: the
//! context is the title's FSR context, made, configured, queried and
//! destroyed by its own code, and a DLSS context (a Streamline viewport) is
//! made beside it. An upscale dispatch is evaluated by DLSS on the host
//! GPU; one that DLSS does not evaluate (no DLSS context, a texture without
//! its host copy, Streamline failing) goes to the title's own `ffxDispatch`,
//! so FSR still makes the picture: never a black screen for want of DLSS.
//! The first dispatches, and every reason one is not evaluated, are logged.
#![allow(non_snake_case)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use super::{
    Configure, Create, CreateUpscale, Destroy, Dispatch, ERROR, Header, OK, OUTPUT, Query,
    ReturnCode, dlss, host_device, record_on_host, upscale_in,
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

/// Each title context (its address) and the DLSS context made beside it.
static CONTEXTS: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());

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

fn contexts() -> std::sync::MutexGuard<'static, Vec<(usize, usize)>> {
    CONTEXTS.lock().unwrap_or_else(|e| e.into_inner())
}

/// The DLSS context beside the title's context `context` points at.
fn beside(context: *mut *mut c_void) -> Option<usize> {
    // SAFETY: the title's context pointer, checked for null.
    let title = (!context.is_null()).then(|| unsafe { *context } as usize)?;
    contexts()
        .iter()
        .find(|&&(t, _)| t == title && title != 0)
        .map(|&(_, ours)| ours)
}

extern "C" fn ffxCreateContext(
    context: *mut *mut c_void,
    desc: *mut Header,
    memory: *const c_void,
) -> ReturnCode {
    let Some(real) = REAL.get() else { return ERROR };
    // SAFETY: the title's own call with its own arguments.
    let r = unsafe { (real.create)(context, desc, memory) };
    // SAFETY: the title's context pointer, checked for null.
    if r != OK || context.is_null() || unsafe { (*context).is_null() } || desc.is_null() {
        return r;
    }
    // SAFETY: the title's chain of descriptions.
    let upscale: Option<CreateUpscale> = unsafe { upscale_in(desc) };
    let (Some(upscale), Some(device)) = (upscale, host_device()) else {
        tracing::warn!("dlss: a FidelityFX context without an upscale description or host device; FSR upscales it");
        return r;
    };
    let mut slot = std::ptr::null_mut();
    if dlss::create(&mut slot, device, &upscale) != OK || slot.is_null() {
        tracing::warn!("dlss: no DLSS context; the title's FSR upscales instead");
        return r;
    }
    // SAFETY: checked non-null above.
    let title = unsafe { *context } as usize;
    contexts().push((title, slot as usize));
    r
}

extern "C" fn ffxDestroyContext(context: *mut *mut c_void, memory: *const c_void) -> ReturnCode {
    let Some(real) = REAL.get() else { return ERROR };
    if let Some(ours) = beside(context) {
        contexts().retain(|&(_, o)| o != ours);
        let mut slot = ours as *mut c_void;
        dlss::destroy(&mut slot);
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

extern "C" fn ffxDispatch(context: *mut *mut c_void, desc: *const Header) -> ReturnCode {
    let Some(real) = REAL.get() else { return ERROR };
    static COUNT: AtomicU32 = AtomicU32::new(0);
    let n = COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    let evaluated = match beside(context) {
        _ if desc.is_null() => Err("no description"),
        None => Err("no DLSS context for this FidelityFX context"),
        Some(ours) => {
            let mut slot = ours as *mut c_void;
            // SAFETY: the title's non-null description; DLSS records on
            // the host list the swap gives it.
            unsafe {
                record_on_host(desc, &[0, 1, 2, OUTPUT], |d| {
                    dlss::dispatch(&mut slot, d)
                })
            }
            .and_then(|r| if r == OK { Ok(()) } else { Err("DLSS failed (see the dlss: error above)") })
        }
    };
    match evaluated {
        Ok(()) => {
            if n <= LOGGED {
                tracing::info!(n, "dlss: upscale dispatch evaluated by DLSS");
            }
            OK
        }
        Err(why) => {
            skipped(n, why);
            // SAFETY: the title's own call with its own arguments.
            unsafe { (real.dispatch)(context, desc) }
        }
    }
}

/// Says that dispatch `n` went to FSR: each of the first ones, then each
/// reason once.
fn skipped(n: u32, why: &'static str) {
    static SEEN: Mutex<Vec<&str>> = Mutex::new(Vec::new());
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let new = !seen.contains(&why);
    if new {
        seen.push(why);
    }
    if n <= LOGGED || new {
        tracing::warn!(n, why, "dlss: upscale dispatch not evaluated; the title's FSR upscales it");
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
        assert!(beside(std::ptr::null_mut()).is_none());
    }
}
