//! NVIDIA DLSS Frame Generation (DLSS-G) through Streamline, on our present
//! (`VICTORY_FRAMEGEN=1`, off by default).
//!
//! After trustno1-coder/VIctory-NVIDIA 6395727 and 22c6be2 (`fg.rs`,
//! `streamline.rs`: Streamline's manual hooking, the factory proxy, the
//! constants, tags and latency markers handed over at each present,
//! `VICTORY_FRAMEGEN_X`), with pepezin-ai/VIctory-NVIDIA 1469efe (the
//! constants and tags come from every upscale the title makes, not only from
//! a DLSS one). All GPL-3.0-or-later; written again here for this tree, whose
//! DLSS answers the title's FidelityFX calls (`super::dlss`, `super::bridge`).
//!
//! Streamline is the one DLSS super resolution uses (`super::dlss::streamline`):
//! `slInit` is called once a process, with DLSS-G, Reflex (and PCL) among its
//! features when this is asked for and its files are in the folder
//! ([`dir`], [`FILES`]). Then:
//!
//! - [`start`] (before the presenter's swap chain is made): Streamline's
//!   support for DLSS-G on this adapter is asked and logged with its reason
//!   and requirements; the DXGI factory is upgraded to Streamline's proxy, so
//!   that the swap chain made from it is the proxy that shows generated
//!   frames at `Present`. Anything missing leaves our plain present.
//! - [`from_upscale`]: what an upscale of the title reads, from every
//!   upscale whichever upscaler does it: the FidelityFX dispatch description
//!   (DLSS, `VICTORY_FFX_DIR`) through [`from_ffx`], and the title's own
//!   temporal upscaler (`VICTORY_OWN_UPSCALER=1`, our default) through
//!   [`own_upscale`], a stand-in for its dispatch method that the loader puts
//!   in its virtual table (`victory_loader::tap_upscale`).
//! - [`for_present`] at the title's present call takes the latest upscale
//!   for that frame (the line of submissions may serve the present later,
//!   when the next frame's upscale has come); [`present_start`] and
//!   [`present_end`] around the proxy's `Present` hand Streamline the frame's
//!   constants, its depth and motion vectors, the colour without the
//!   interface (Series S's scene plane) and the interface plane, and the
//!   latency markers.
//!
//! The title gives no camera matrices: they are made from its field of view,
//! near and far planes and the render size, the camera standing still (its
//! motion is in the motion vectors, in render pixels).
//!
//! On RTX 20 and 30 cards Streamline says DLSS-G is not supported unless
//! `nvngx_dlssg.dll` is replaced (dlssg-to-fsr3, which runs FSR 3 frame
//! generation behind DLSS-G's calls): [`start`] logs Streamline's answer
//! and its reason; `VICTORY_FRAMEGEN_FORCE=1` goes on anyway.
//!
//! Switches (all read here):
//! - `VICTORY_FRAMEGEN=1`: on (off by default).
//! - `VICTORY_FRAMEGEN_X=2|3|4`: frames shown per rendered one (2).
//! - `VICTORY_FRAMEGEN_DIR`: Streamline's folder, `VICTORY_DLSS_DIR` without
//!   it. With DLSS super resolution on, `VICTORY_DLSS_DIR` holds both; with
//!   the title's own upscaler, `VICTORY_DLSS_MODE=off` keeps DLSS super
//!   resolution off while `VICTORY_DLSS_DIR` serves frame generation.
//! - `VICTORY_FRAMEGEN_FORCE=1`: go on when Streamline says DLSS-G is not
//!   supported.
//! - `VICTORY_FRAMEGEN_HUDLESS=0`: no colour without the interface and no
//!   interface tagged.
//! - `VICTORY_FRAMEGEN_REFLEX=0`: no Reflex sleep after the present (Reflex
//!   stays in low-latency mode, which DLSS-G needs).
//! - `VICTORY_FRAMEGEN_MV=x,y`: multipliers of the motion-vector scale (1,1).
//! - `VICTORY_FRAMEGEN_JITTER=x,y`: multipliers of the FidelityFX path's
//!   jitter (1,1); `VICTORY_FRAMEGEN_OWN_JITTER=x,y`: of the own upscaler's
//!   (-1,-1).
//! - `VICTORY_FRAMEGEN_DEPTH_INVERTED=0`: the own upscaler's depth taken as
//!   not inverted.
//! - `VICTORY_TEXTURE_ROOM_PERCENT` defaults to 80 with frame generation
//!   asked for (`crate::resource`).

use std::ffi::{c_char, c_void};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use windows::Win32::Graphics::Direct3D12::{ID3D12Device, ID3D12Resource};
use windows::Win32::Graphics::Dxgi::IDXGIFactory2;
use windows::core::Interface;

use super::dlss::{
    BUFFER_DEPTH, BUFFER_MOTION_VECTORS, Base, CONSTANTS, Constants, Extent, FALSE, RESOURCE_TAG,
    Resource, ResourceTag, SlResult, StructType, Streamline, TRUE, Viewport,
};
use super::{DISPATCH_UPSCALE, DispatchUpscale, Header};
use crate::resource::HostTexture;

/// The files frame generation needs next to `sl.interposer.dll`.
pub(super) const FILES: [&str; 6] = [
    "sl.interposer.dll",
    "sl.common.dll",
    "sl.dlss_g.dll",
    "nvngx_dlssg.dll",
    "sl.reflex.dll",
    "sl.pcl.dll",
];

// `sl::Feature`.
const FEATURE_REFLEX: u32 = 3;
const FEATURE_PCL: u32 = 4;
const FEATURE_DLSS_G: u32 = 1000;
/// Frame generation's features for `slInit` (sl.common and sl.pcl are added
/// by Streamline itself; sl.dlss_g requires sl.reflex).
pub(super) const FEATURES: [u32; 2] = [FEATURE_DLSS_G, FEATURE_REFLEX];

// `sl::BufferType`.
const BUFFER_HUDLESS_COLOR: u32 = 2;
const BUFFER_UI_COLOR_AND_ALPHA: u32 = 23;
/// `sl::ResourceLifecycle::eValidUntilPresent`.
const VALID_UNTIL_PRESENT: i32 = 1;
// `sl::PCLMarker`.
const SIMULATION_START: u32 = 0;
const SIMULATION_END: u32 = 1;
const RENDER_SUBMIT_START: u32 = 2;
const RENDER_SUBMIT_END: u32 = 3;
const PRESENT_START: u32 = 4;
const PRESENT_END: u32 = 5;
// `sl::DLSSGMode`, `sl::DLSSGFlags`, `sl::ReflexMode`.
const DLSSG_OFF: u32 = 0;
const DLSSG_ON: u32 = 1;
const DLSSG_RETAIN_RESOURCES_WHEN_OFF: u32 = 1 << 3;
const REFLEX_LOW_LATENCY: i32 = 1;

/// The viewport frame generation's constants, tags and options go to: apart
/// from DLSS super resolution's (one per upscale context, from 0).
const VIEWPORT: u32 = 0x4647;

/// `sl::Result` names (`sl_result.h`), for the log.
const RESULTS: [&str; 40] = [
    "eOk",
    "eErrorIO",
    "eErrorDriverOutOfDate",
    "eErrorOSOutOfDate",
    "eErrorOSDisabledHWS",
    "eErrorDeviceNotCreated",
    "eErrorNoSupportedAdapterFound",
    "eErrorAdapterNotSupported",
    "eErrorNoPlugins",
    "eErrorVulkanAPI",
    "eErrorDXGIAPI",
    "eErrorD3DAPI",
    "eErrorNRDAPI",
    "eErrorNVAPI",
    "eErrorReflexAPI",
    "eErrorNGXFailed",
    "eErrorJSONParsing",
    "eErrorMissingProxy",
    "eErrorMissingResourceState",
    "eErrorInvalidIntegration",
    "eErrorMissingInputParameter",
    "eErrorNotInitialized",
    "eErrorComputeFailed",
    "eErrorInitNotCalled",
    "eErrorExceptionHandler",
    "eErrorInvalidParameter",
    "eErrorMissingConstants",
    "eErrorDuplicatedConstants",
    "eErrorMissingOrInvalidAPI",
    "eErrorCommonConstantsMissing",
    "eErrorUnsupportedInterface",
    "eErrorFeatureMissing",
    "eErrorFeatureNotSupported",
    "eErrorFeatureMissingHooks",
    "eErrorFeatureFailedToLoad",
    "eErrorFeatureWrongPriority",
    "eErrorFeatureMissingDependency",
    "eErrorFeatureManagerInvalidState",
    "eErrorInvalidState",
    "eWarnOutOfVRAM",
];

fn result_name(r: SlResult) -> &'static str {
    usize::try_from(r).ok().and_then(|i| RESULTS.get(i).copied()).unwrap_or("unknown")
}

// Streamline structures for frame generation (`sl_core_types.h`,
// `sl_dlss_g.h`, `sl_reflex.h`, SDK 2.14), sizes as MSVC gives them.

/// `sl::AdapterInfo`.
#[repr(C)]
struct AdapterInfo {
    base: Base,
    luid: *const u8,
    luid_size: u32,
    vk_physical_device: *mut c_void,
}
const _: () = assert!(size_of::<AdapterInfo>() == 56);
const ADAPTER_INFO: StructType =
    StructType(0x0677_315f, 0xa746, 0x4492, [0x9f, 0x42, 0xcb, 0x61, 0x42, 0xc9, 0xc3, 0xd4]);

/// `sl::Version`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Version {
    major: u32,
    minor: u32,
    build: u32,
}

/// `sl::FeatureRequirements` (version 2).
#[repr(C)]
struct FeatureRequirements {
    base: Base,
    flags: u32,
    max_cpu_threads: u32,
    max_viewports: u32,
    required_tags_count: u32,
    required_tags: *const u32,
    os_detected: Version,
    os_required: Version,
    driver_detected: Version,
    driver_required: Version,
    vk: [u32; 19],
}
const _: () = assert!(size_of::<FeatureRequirements>() == 184);
const _: () = assert!(std::mem::offset_of!(FeatureRequirements, os_detected) == 56);
const FEATURE_REQUIREMENTS: StructType =
    StructType(0x6671_4097, 0xac6d, 0x4bc6, [0x89, 0x15, 0x1e, 0x0f, 0x55, 0xa6, 0xb6, 0x1f]);

/// `sl::DLSSGOptions` (version 5).
#[repr(C)]
struct DlssgOptions {
    base: Base,
    mode: u32,
    frames_to_generate: u32,
    flags: u32,
    dynamic_res: [u32; 2],
    back_buffers: u32,
    mvec_depth: [u32; 2],
    colour: [u32; 2],
    formats: [u32; 5],
    on_error: *const c_void,
    reserved15: u8,
    queue_parallelism: u32,
    ui_recomposition: u8,
    dynamic_target_frame_rate: f32,
}
const _: () = assert!(size_of::<DlssgOptions>() == 120);
const _: () = assert!(std::mem::offset_of!(DlssgOptions, on_error) == 96);
const _: () = assert!(std::mem::offset_of!(DlssgOptions, queue_parallelism) == 108);
const DLSSG_OPTIONS: StructType =
    StructType(0xfac5_f1cb, 0x2dfd, 0x4f36, [0xa1, 0xe6, 0x3a, 0x9e, 0x86, 0x52, 0x56, 0xc5]);

impl DlssgOptions {
    fn new(on: bool, frames_to_generate: u32) -> Self {
        DlssgOptions {
            base: Base::new(DLSSG_OPTIONS, 5),
            mode: if on { DLSSG_ON } else { DLSSG_OFF },
            frames_to_generate: frames_to_generate.max(1),
            flags: DLSSG_RETAIN_RESOURCES_WHEN_OFF,
            dynamic_res: [0; 2],
            back_buffers: 0,
            mvec_depth: [0; 2],
            colour: [0; 2],
            formats: [0; 5],
            on_error: std::ptr::null(),
            // `sl::Boolean::eInvalid`, as the header's default.
            reserved15: 2,
            queue_parallelism: 0,
            ui_recomposition: FALSE,
            dynamic_target_frame_rate: 0.0,
        }
    }
}

/// `sl::DLSSGState` (version 4).
#[repr(C)]
struct DlssgState {
    base: Base,
    estimated_vram: u64,
    status: u32,
    min_width_or_height: u32,
    frames_presented: u32,
    frames_to_generate_max: u32,
    reserved4: u8,
    vsync_support: u8,
    fence: *mut c_void,
    fence_value: u64,
    dynamic_mfg: u8,
}
const _: () = assert!(size_of::<DlssgState>() == 88);
const _: () = assert!(std::mem::offset_of!(DlssgState, fence) == 64);
const DLSSG_STATE: StructType =
    StructType(0xcc8a_c8e1, 0xa179, 0x44f5, [0x97, 0xfa, 0xe7, 0x41, 0x12, 0xf9, 0xbc, 0x61]);

impl DlssgState {
    fn new() -> Self {
        DlssgState {
            base: Base::new(DLSSG_STATE, 4),
            estimated_vram: 0,
            status: 0,
            min_width_or_height: 0,
            frames_presented: 0,
            frames_to_generate_max: 0,
            reserved4: 0,
            vsync_support: 0,
            fence: std::ptr::null_mut(),
            fence_value: 0,
            dynamic_mfg: 0,
        }
    }
}

/// `sl::ReflexOptions` (version 1).
#[repr(C)]
struct ReflexOptions {
    base: Base,
    mode: i32,
    frame_limit_us: u32,
    use_markers_to_optimize: bool,
    virtual_key: u16,
    thread: u32,
}
const _: () = assert!(size_of::<ReflexOptions>() == 48);
const REFLEX_OPTIONS: StructType =
    StructType(0xf03a_f81a, 0x6d0b, 0x4902, [0xa6, 0x51, 0xc4, 0x96, 0x5e, 0x21, 0x54, 0x34]);

type DlssgSetOptions = unsafe extern "C" fn(*const Viewport, *const DlssgOptions) -> SlResult;
type DlssgGetState =
    unsafe extern "C" fn(*const Viewport, *mut DlssgState, *const DlssgOptions) -> SlResult;
type ReflexSetOptions = unsafe extern "C" fn(*const ReflexOptions) -> SlResult;
type ReflexSleep = unsafe extern "C" fn(*const c_void) -> SlResult;
/// `slPCLSetMarker` (or, in older Streamline, `slReflexSetMarker`).
type SetMarker = unsafe extern "C" fn(u32, *const c_void) -> SlResult;

/// Whether frame generation is asked for (`VICTORY_FRAMEGEN=1`).
pub fn wanted() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_FRAMEGEN").is_some_and(|v| v == "1"))
}

/// Frames shown for each one rendered, `VICTORY_FRAMEGEN_X` (2 to 4; 2,
/// one generated frame, without it). Never more than the card generates
/// (DLSS-G's own maximum, [`present_start`]).
fn multiplier() -> u32 {
    static X: OnceLock<u32> = OnceLock::new();
    *X.get_or_init(|| {
        let Ok(v) = std::env::var("VICTORY_FRAMEGEN_X") else {
            return 2;
        };
        match v.trim().trim_end_matches(['x', 'X']).parse::<u32>() {
            Ok(x @ 2..=4) => x,
            _ => {
                tracing::warn!(value = %v, "framegen: VICTORY_FRAMEGEN_X is 2, 3 or 4; 2 is used");
                2
            }
        }
    })
}

fn env_on(name: &str, default: bool) -> bool {
    std::env::var(name).map_or(default, |v| v.trim() != "0")
}

/// `x,y` multipliers from `name`, `default` without it.
fn env_pair(name: &str, default: [f32; 2]) -> [f32; 2] {
    let Ok(v) = std::env::var(name) else {
        return default;
    };
    let parts: Vec<f32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    match parts[..] {
        [x, y] => [x, y],
        [x] => [x, x],
        _ => default,
    }
}

/// The folder of Streamline's files for frame generation:
/// `VICTORY_FRAMEGEN_DIR`, else `VICTORY_DLSS_DIR`. (With DLSS super
/// resolution on, Streamline is loaded from `VICTORY_DLSS_DIR`, and that
/// folder must hold both: `super::dlss::streamline_dir`.)
pub(super) fn dir() -> Option<std::path::PathBuf> {
    std::env::var_os("VICTORY_FRAMEGEN_DIR")
        .or_else(|| std::env::var_os("VICTORY_DLSS_DIR"))
        .map(std::path::PathBuf::from)
}

/// The files of [`FILES`] not in `dir`.
fn missing_files(dir: &std::path::Path) -> Vec<&'static str> {
    FILES.into_iter().filter(|f| !dir.join(f).is_file()).collect()
}

/// Whether frame generation is asked for and Streamline's folder holds its
/// files; the files missing are said once, and frame generation is then
/// off (our plain present, as without it).
pub(super) fn files_ready() -> bool {
    static READY: OnceLock<bool> = OnceLock::new();
    *READY.get_or_init(|| {
        if !wanted() {
            return false;
        }
        let Some(dir) = super::dlss::streamline_dir() else {
            tracing::error!(
                "framegen: VICTORY_FRAMEGEN=1 without VICTORY_FRAMEGEN_DIR or VICTORY_DLSS_DIR (Streamline's folder); frame generation off"
            );
            return false;
        };
        let missing = missing_files(&dir);
        if missing.is_empty() {
            tracing::info!(dir = %dir.display(), files = ?FILES, "framegen: Streamline's frame generation files are there");
            return true;
        }
        tracing::error!(
            dir = %dir.display(),
            ?missing,
            needed = ?FILES,
            "framegen: Streamline's folder lacks files frame generation needs; frame generation off, plain present"
        );
        false
    })
}

/// Frame generation running: Streamline's functions and what was last
/// given to DLSS-G.
struct Fg {
    sl: &'static Streamline,
    set_options: DlssgSetOptions,
    get_state: DlssgGetState,
    reflex_sleep: Option<ReflexSleep>,
    marker: Option<SetMarker>,
    /// Frames to generate per rendered one (1: 2x).
    frames: u32,
    /// DLSS-G's mode as last set: none before the first present or after a
    /// resize.
    on: Option<bool>,
    /// Presents in a row without a new upscale.
    stale: u32,
    /// DLSSGState's status as last seen.
    status: u32,
    told: u32,
}

/// Set by [`start`]: frame generation running, if it could start.
static FG: OnceLock<Option<Mutex<Fg>>> = OnceLock::new();
/// Our swap chain is Streamline's proxy (the factory was upgraded).
static PROXY: AtomicBool = AtomicBool::new(false);

fn fg() -> Option<&'static Mutex<Fg>> {
    FG.get().and_then(|f| f.as_ref()).filter(|_| PROXY.load(Ordering::Relaxed))
}

/// Whether frame generation runs (Streamline presents through its proxy).
pub(crate) fn active() -> bool {
    fg().is_some()
}

/// Whether upscales are worth reading for frame generation: it is asked for
/// and did not fail to start (before the presenter is made, it may yet).
fn taking() -> bool {
    wanted()
        && match FG.get() {
            None => true,
            Some(fg) => fg.is_some() && PROXY.load(Ordering::Relaxed),
        }
}

/// Streamline's answer for `feature` on the adapter of `luid`, logged.
fn supported(sl: &Streamline, feature: u32, name: &str, luid: &[u8; 8]) -> SlResult {
    let adapter = AdapterInfo {
        base: Base::new(ADAPTER_INFO, 1),
        luid: luid.as_ptr(),
        luid_size: 8,
        vk_physical_device: std::ptr::null_mut(),
    };
    // SAFETY: an adapter description that outlives the call.
    let r = unsafe { (sl.is_feature_supported)(feature, (&raw const adapter).cast()) };
    if r == 0 {
        tracing::info!(feature = name, "framegen: Streamline says {name} is supported on this GPU");
    } else {
        tracing::warn!(feature = name, result = result_name(r), code = r, "framegen: Streamline says {name} is NOT supported on this GPU");
    }
    if let Some(get) = sl.get_feature_requirements {
        // SAFETY: a plain structure of SL's, filled by the call.
        let mut req: FeatureRequirements = unsafe { std::mem::zeroed() };
        req.base = Base::new(FEATURE_REQUIREMENTS, 2);
        // SAFETY: as above.
        let q = unsafe { get(feature, (&raw mut req).cast()) };
        if q == 0 {
            let tags = if req.required_tags.is_null() {
                Vec::new()
            } else {
                // SAFETY: SL's array of that many tags.
                unsafe { std::slice::from_raw_parts(req.required_tags, req.required_tags_count as usize) }.to_vec()
            };
            tracing::info!(
                feature = name,
                flags = format_args!("{:#x} (0x10: hardware-accelerated GPU scheduling required)", req.flags),
                os = ?(req.os_detected, req.os_required),
                driver = ?(req.driver_detected, req.driver_required),
                required_tags = ?tags,
                "framegen: Streamline's requirements (detected, required)"
            );
        }
    }
    r
}

/// A feature function of Streamline's, or why not.
fn feature_function(sl: &Streamline, feature: u32, name: &[u8]) -> Result<*mut c_void, String> {
    let mut f = std::ptr::null_mut();
    // SAFETY: a nul-terminated name and an output pointer.
    let r = unsafe { (sl.get_feature_function)(feature, name.as_ptr().cast::<c_char>(), &mut f) };
    if r == 0 && !f.is_null() {
        Ok(f)
    } else {
        Err(format!(
            "{} not found ({})",
            String::from_utf8_lossy(&name[..name.len() - 1]),
            result_name(r)
        ))
    }
}

fn init(device: &ID3D12Device) -> Result<Fg, String> {
    if !files_ready() {
        return Err("files missing (see above)".into());
    }
    let sl = super::dlss::streamline(device.as_raw())
        .ok_or("Streamline did not start (see the streamline: lines above)")?;
    if !sl.framegen {
        return Err("Streamline was started without frame generation's features".into());
    }
    // SAFETY: a plain query of a live device.
    let luid = unsafe { device.GetAdapterLuid() };
    let mut bytes = [0u8; 8];
    bytes[..4].copy_from_slice(&luid.LowPart.to_le_bytes());
    bytes[4..].copy_from_slice(&luid.HighPart.to_le_bytes());
    let dlssg = supported(sl, FEATURE_DLSS_G, "DLSS Frame Generation", &bytes);
    let _ = supported(sl, FEATURE_REFLEX, "Reflex", &bytes);
    let _ = supported(sl, FEATURE_PCL, "PC Latency", &bytes);
    if dlssg != 0 {
        let force = env_on("VICTORY_FRAMEGEN_FORCE", false);
        tracing::warn!(
            result = result_name(dlssg),
            code = dlssg,
            force,
            "framegen: DLSS-G not supported here. On RTX 20/30 this is expected unless nvngx_dlssg.dll is dlssg-to-fsr3's (FSR 3 frame generation behind DLSS-G's calls); RTX 40 and later: check the driver and hardware-accelerated GPU scheduling (requirements above). VICTORY_FRAMEGEN_FORCE=1 goes on anyway"
        );
        if !force {
            if let Some(unload) = sl.set_feature_loaded {
                // SAFETY: Streamline's call; DLSS-G unhooked, nothing else.
                let _ = unsafe { unload(FEATURE_DLSS_G, false) };
            }
            return Err(format!("DLSS-G not supported ({})", result_name(dlssg)));
        }
    }
    let set_options = feature_function(sl, FEATURE_DLSS_G, b"slDLSSGSetOptions\0")?;
    let get_state = feature_function(sl, FEATURE_DLSS_G, b"slDLSSGGetState\0")?;
    let reflex_options = feature_function(sl, FEATURE_REFLEX, b"slReflexSetOptions\0")?;
    let reflex_sleep = feature_function(sl, FEATURE_REFLEX, b"slReflexSleep\0").ok();
    let marker = feature_function(sl, FEATURE_PCL, b"slPCLSetMarker\0")
        .or_else(|_| feature_function(sl, FEATURE_REFLEX, b"slReflexSetMarker\0"))
        .ok();
    if marker.is_none() {
        tracing::warn!("framegen: no latency marker function (slPCLSetMarker, slReflexSetMarker); DLSS-G may report Reflex missing");
    }
    // SAFETY: Streamline's functions, of these types (`sl_dlss_g.h`,
    // `sl_reflex.h`, `sl_pcl.h`).
    let (set_options, get_state, reflex_options, reflex_sleep, marker) = unsafe {
        (
            std::mem::transmute::<*mut c_void, DlssgSetOptions>(set_options),
            std::mem::transmute::<*mut c_void, DlssgGetState>(get_state),
            std::mem::transmute::<*mut c_void, ReflexSetOptions>(reflex_options),
            reflex_sleep.map(|f| std::mem::transmute::<*mut c_void, ReflexSleep>(f)),
            marker.map(|f| std::mem::transmute::<*mut c_void, SetMarker>(f)),
        )
    };
    // DLSS-G needs Reflex on (status "Reflex not detected" otherwise).
    let reflex = ReflexOptions {
        base: Base::new(REFLEX_OPTIONS, 1),
        mode: REFLEX_LOW_LATENCY,
        frame_limit_us: 0,
        use_markers_to_optimize: false,
        virtual_key: 0,
        thread: 0,
    };
    // SAFETY: options that outlive the call.
    let r = unsafe { reflex_options(&reflex) };
    if r != 0 {
        tracing::warn!(result = result_name(r), "framegen: Reflex low latency not set");
    }
    // What this card generates at most (DLSS 4 multi frame generation on RTX
    // 50: up to 3 here; 1 elsewhere).
    let mut state = DlssgState::new();
    let viewport = Viewport::new(VIEWPORT);
    // SAFETY: structures that outlive the call.
    let r = unsafe { get_state(&viewport, &mut state, std::ptr::null()) };
    let most = if r == 0 && state.frames_to_generate_max > 0 { state.frames_to_generate_max } else { 1 };
    let asked = multiplier() - 1;
    let frames = asked.min(most);
    if frames != asked {
        tracing::warn!(asked = asked + 1, given = frames + 1, "framegen: VICTORY_FRAMEGEN_X is more than this card generates; its most is used");
    }
    tracing::info!(
        shown_per_rendered = frames + 1,
        most = most + 1,
        status = state.status,
        min_size = state.min_width_or_height,
        "framegen: DLSS-G ready"
    );
    Ok(Fg {
        sl,
        set_options,
        get_state,
        reflex_sleep: reflex_sleep.filter(|_| env_on("VICTORY_FRAMEGEN_REFLEX", true)),
        marker,
        frames,
        on: None,
        stale: 0,
        status: 0,
        told: 0,
    })
}

/// Before the presenter's swap chain is made: frame generation started
/// (once), and the factory to make the swap chain from: Streamline's proxy
/// if it started, `factory` as it is otherwise.
pub(crate) fn start(device: &ID3D12Device, factory: IDXGIFactory2) -> IDXGIFactory2 {
    if !wanted() {
        return factory;
    }
    let started = FG.get_or_init(|| match init(device) {
        Ok(fg) => Some(Mutex::new(fg)),
        Err(why) => {
            tracing::warn!(why, "framegen: frame generation not started; plain present");
            None
        }
    });
    let Some(fg) = started else { return factory };
    let fg = fg.lock().unwrap_or_else(|e| e.into_inner());
    let Some(upgrade) = fg.sl.upgrade_interface else {
        tracing::warn!("framegen: no slUpgradeInterface; plain present");
        return factory;
    };
    let mut raw = factory.as_raw();
    // SAFETY: a live factory; the call puts a new proxy (with its own
    // reference) in `raw` and leaves ours to the native one.
    let r = unsafe { upgrade(&mut raw) };
    if r != 0 || raw.is_null() || raw == factory.as_raw() {
        tracing::warn!(result = result_name(r), "framegen: the DXGI factory was not upgraded to Streamline's proxy; plain present");
        return factory;
    }
    PROXY.store(true, Ordering::Relaxed);
    tracing::info!(shown_per_rendered = fg.frames + 1, "framegen: our swap chain is made through Streamline's proxy; frame generation on");
    // SAFETY: SL's proxy, an IDXGIFactory2 with one reference, now ours.
    unsafe { IDXGIFactory2::from_raw(raw) }
}

/// What an upscale of the title read: its depth and motion vectors (host
/// copies, held) and the numbers frame generation's constants are made of.
pub(crate) struct Inputs {
    depth: Arc<HostTexture>,
    motion: Arc<HostTexture>,
    /// Render size: the extent of depth and motion vectors.
    render: [u32; 2],
    /// Jitter in render pixels, as DLSS takes it.
    jitter: [f32; 2],
    /// Motion-vector scale to [-1, 1] (pixels over the render size).
    mvec_scale: [f32; 2],
    near: f32,
    far: f32,
    /// Vertical field of view, radians.
    fov: f32,
    inverted: bool,
    reset: bool,
}

// SAFETY: D3D12 resources are free-threaded; the host copies are only read
// (their state word is atomic).
unsafe impl Send for Inputs {}
// SAFETY: as above.
unsafe impl Sync for Inputs {}

/// The latest upscale's inputs, and whether one came since the last present.
static LATEST: Mutex<Option<Arc<Inputs>>> = Mutex::new(None);
static FRESH: AtomicBool = AtomicBool::new(false);

/// An upscale the title made: depth, motion vectors and numbers become the
/// latest, which the next present takes ([`for_present`]). `motion_pixels`
/// is the scale that turns the motion vectors into render pixels; `render`
/// the render size, or zero for the textures' own.
#[allow(clippy::too_many_arguments)]
fn from_upscale(
    source: &'static str,
    depth: crate::object::This,
    motion: crate::object::This,
    render: [u32; 2],
    jitter: [f32; 2],
    motion_pixels: [f32; 2],
    near_far: [f32; 2],
    fov: f32,
    inverted: bool,
    reset: bool,
) {
    let (Some(depth), Some(motion)) = (crate::resource::host_copy(depth), crate::resource::host_copy(motion)) else {
        static TOLD: std::sync::Once = std::sync::Once::new();
        TOLD.call_once(|| tracing::warn!(source, "framegen: an upscale's depth or motion vectors have no host texture; not tagged"));
        return;
    };
    // SAFETY: plain queries of live textures.
    let (d, m) = unsafe { (depth.texture.GetDesc(), motion.texture.GetDesc()) };
    let own = [(d.Width as u32).min(m.Width as u32), d.Height.min(m.Height)];
    let render = if render[0] == 0 || render[1] == 0 { own } else { [render[0].min(own[0]), render[1].min(own[1])] };
    let scale = |s: f32| if s == 0.0 { 1.0 } else { s };
    let mv = env_pair("VICTORY_FRAMEGEN_MV", [1.0, 1.0]);
    let [a, b] = near_far;
    let inputs = Inputs {
        depth,
        motion,
        render,
        jitter,
        mvec_scale: [
            mv[0] * scale(motion_pixels[0]) / render[0].max(1) as f32,
            mv[1] * scale(motion_pixels[1]) / render[1].max(1) as f32,
        ],
        near: a.min(b).max(1e-3),
        far: a.max(b).min(1e7).max(a.min(b).max(1e-3) + 1.0),
        fov: if fov > 0.1 && fov < 3.0 { fov } else { 60f32.to_radians() },
        inverted,
        reset,
    };
    static FIRST: Mutex<Vec<&str>> = Mutex::new(Vec::new());
    {
        let mut first = FIRST.lock().unwrap_or_else(|e| e.into_inner());
        if !first.contains(&source) {
            first.push(source);
            tracing::info!(
                source,
                render = ?inputs.render,
                depth = ?(d.Width, d.Height, d.Format.0),
                motion = ?(m.Width, m.Height, m.Format.0),
                jitter = ?inputs.jitter,
                mvec_scale = ?inputs.mvec_scale,
                near = inputs.near,
                far = inputs.far,
                fov = inputs.fov,
                inverted,
                "framegen: first upscale whose depth and motion vectors are handed to DLSS-G"
            );
        }
    }
    *LATEST.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(inputs));
    FRESH.store(true, Ordering::Relaxed);
}

/// A FidelityFX upscale dispatch of the title's (its description, before
/// its objects are swapped for the host's), from every path that answers
/// them (DLSS, AMD's PC upscaler): its jitter, motion-vector scale (render
/// pixels), camera and textures as frame generation's inputs.
///
/// # Safety
///
/// `desc` is null or the title's valid description.
pub(super) unsafe fn from_ffx(desc: *const Header) {
    if !taking() || desc.is_null() {
        return;
    }
    // SAFETY: the caller's description.
    if unsafe { (*desc).kind } != DISPATCH_UPSCALE {
        return;
    }
    // SAFETY: a description of this type.
    let d = unsafe { &*desc.cast::<DispatchUpscale>() };
    let (depth, motion) = (d.resources[1].resource, d.resources[2].resource);
    if depth.is_null() || motion.is_null() {
        return;
    }
    let [near, far, fov, _] = d.camera;
    let jitter = env_pair("VICTORY_FRAMEGEN_JITTER", [1.0, 1.0]);
    from_upscale(
        "the title's FidelityFX upscale",
        depth as crate::object::This,
        motion as crate::object::This,
        d.render_size,
        [jitter[0] * d.jitter[0], jitter[1] * d.jitter[1]],
        d.motion_vector_scale,
        [near, far],
        fov,
        // The title names the far plane first with its inverted depth.
        near > far,
        d.reset,
    );
}

/// The title's own temporal upscaler's dispatch method (1.0.2.1: slot 12 of
/// its virtual table), which the loader puts [`own_upscale`] in front of.
static OWN_DISPATCH: AtomicUsize = AtomicUsize::new(0);

/// Where the title's own upscaler's dispatch method is (given by the loader
/// before it puts [`own_upscale`] in its place).
pub fn set_own_upscale_original(address: usize) {
    OWN_DISPATCH.store(address, Ordering::Relaxed);
}

/// `Upscaler::Dispatch(const Inputs&)` of the title's own temporal
/// upscaler (renderer 6), returning its 56-byte result through `out`.
type OwnDispatch = unsafe extern "system" fn(*mut c_void, *mut c_void, *const u8) -> *mut c_void;

/// In place of the title's own upscaler's dispatch method: the method runs
/// as it is, then frame generation takes the frame's depth, motion vectors,
/// jitter and camera from the same inputs.
///
/// The inputs (1.0.2.1, read from both upscalers' dispatch methods, the
/// FidelityFX one building its `ffxDispatchDescUpscale` from them): +0x00
/// colour, +0x08 depth, +0x28 motion vectors (texture handles whose +0x08
/// object holds the D3D12 resource at +0x20), +0x3d reset, +0x40 near and
/// far, +0x48 vertical field of view. The motion vectors are in render
/// pixels (the FidelityFX path's scale is 1.0). The jitter is the own
/// upscaler's, which its dispatch keeps at +0xf8 of itself (the FidelityFX
/// path hands FSR the opposite of its own): `VICTORY_FRAMEGEN_OWN_JITTER`
/// (`x,y` multipliers, default `-1,-1`). Its depth is inverted
/// (`VICTORY_FRAMEGEN_DEPTH_INVERTED=0`: not).
pub extern "system" fn own_upscale(this: *mut c_void, out: *mut c_void, inputs: *const u8) -> *mut c_void {
    let original = OWN_DISPATCH.load(Ordering::Relaxed);
    // SAFETY: the title's own method, with the arguments it was called with
    // (the loader only puts this in its place after setting `original`).
    let r = unsafe { std::mem::transmute::<usize, OwnDispatch>(original)(this, out, inputs) };
    if !this.is_null() && !inputs.is_null() {
        // SAFETY: the method's own object and inputs, which it just read.
        unsafe { own_inputs(this.cast(), inputs) };
    }
    r
}

/// [`own_upscale`]'s reading of the inputs.
///
/// # Safety
///
/// `this` and `inputs` are the title's upscaler and its dispatch inputs.
unsafe fn own_inputs(this: *const u8, inputs: *const u8) {
    if !taking() {
        return;
    }
    // SAFETY (every read): fields of the title's structures, as above.
    let texture = |at: usize| unsafe {
        let handle = inputs.add(at).cast::<*const u8>().read_unaligned();
        if handle.is_null() {
            return std::ptr::null_mut();
        }
        let object = handle.add(8).cast::<*const u8>().read_unaligned();
        if object.is_null() {
            return std::ptr::null_mut();
        }
        object.add(0x20).cast::<*mut c_void>().read_unaligned()
    };
    let float = |base: *const u8, at: usize| unsafe { base.add(at).cast::<f32>().read_unaligned() };
    let (depth, motion) = (texture(0x08), texture(0x28));
    if depth.is_null() || motion.is_null() {
        return;
    }
    let sign = env_pair("VICTORY_FRAMEGEN_OWN_JITTER", [-1.0, -1.0]);
    let jitter = [sign[0] * float(this, 0xf8), sign[1] * float(this, 0xfc)];
    // SAFETY: as above.
    let reset = unsafe { inputs.add(0x3d).read() } != 0;
    from_upscale(
        "the title's own upscaler",
        depth as crate::object::This,
        motion as crate::object::This,
        [0, 0],
        jitter,
        [1.0, 1.0],
        [float(inputs, 0x40), float(inputs, 0x44)],
        float(inputs, 0x48),
        env_on("VICTORY_FRAMEGEN_DEPTH_INVERTED", true),
        reset,
    );
}

/// What a present hands frame generation: the latest upscale's inputs, and
/// whether one came since the present before.
#[derive(Clone)]
pub(crate) struct ForPresent {
    inputs: Option<Arc<Inputs>>,
    fresh: bool,
}

/// At the title's present call: the inputs of the frame it presents.
pub(crate) fn for_present() -> Option<ForPresent> {
    if !taking() {
        return None;
    }
    Some(ForPresent {
        inputs: LATEST.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        fresh: FRESH.swap(false, Ordering::Relaxed),
    })
}

/// With frame generation running, the title's frame being made, as
/// Streamline's frame index (the presents so far plus one): DLSS super
/// resolution numbers its frames with it then, as frame generation does.
pub(super) fn frame_index() -> Option<u32> {
    active().then(|| (crate::queue::presented() + 1) as u32)
}

fn new_token(fg: &Fg, index: u32) -> Option<*mut c_void> {
    let mut token = std::ptr::null_mut();
    // SAFETY: an output pointer and an index that outlive the call.
    let r = unsafe { (fg.sl.get_new_frame_token)(&mut token, &index) };
    (r == 0 && !token.is_null()).then_some(token)
}

fn mark(fg: &Fg, marker: u32, token: *mut c_void) {
    if let Some(set) = fg.marker {
        // SAFETY: a token of Streamline's.
        let _ = unsafe { set(marker, token) };
    }
}

/// The host DXGI format a typeless one is read as, for Streamline's views.
fn typed(format: u32) -> u32 {
    match format {
        // R32G8X24_TYPELESS, D32_FLOAT_S8X24_UINT -> R32_FLOAT_X8X24_TYPELESS
        19 | 20 => 21,
        // R32_TYPELESS, D32_FLOAT -> R32_FLOAT
        39 | 40 => 41,
        // R24G8_TYPELESS, D24_UNORM_S8_UINT -> R24_UNORM_X8_TYPELESS
        44 | 45 => 46,
        // R16_TYPELESS, D16_UNORM -> R16_UNORM
        53 | 55 => 56,
        // R16G16_TYPELESS -> R16G16_FLOAT
        33 => 34,
        // R16G16B16A16_TYPELESS -> R16G16B16A16_FLOAT
        9 => 10,
        // R32G32_TYPELESS -> R32G32_FLOAT
        15 => 16,
        f => f,
    }
}

fn sl_resource(texture: &ID3D12Resource, state: u32) -> Resource {
    let state = crate::host::legal_state(texture, state);
    let mut r = Resource::texture(texture.as_raw(), state);
    // SAFETY: a plain query.
    let d = unsafe { texture.GetDesc() };
    r.width = d.Width as u32;
    r.height = d.Height;
    r.mip_levels = u32::from(d.MipLevels);
    r.array_layers = u32::from(d.DepthOrArraySize);
    let format = d.Format.0 as u32;
    if typed(format) != format {
        r.native_format = typed(format);
    }
    r
}

/// The camera's projection for a render size, vertical field of view and
/// planes (row-major, row vectors, as `sl_consts.h` has them; no jitter),
/// and its inverse.
fn projection(render: [u32; 2], fov: f32, near: f32, far: f32, inverted: bool) -> ([f32; 16], [f32; 16]) {
    let (w, h) = (render[0].max(1) as f32, render[1].max(1) as f32);
    let (n, f) = (near, far);
    let ys = 1.0 / (fov / 2.0).tan();
    let xs = ys / (w / h);
    let (c, d) = if inverted { (n / (n - f), -n * f / (n - f)) } else { (f / (f - n), -n * f / (f - n)) };
    #[rustfmt::skip]
    let view_to_clip = [
        xs, 0.0, 0.0, 0.0,
        0.0, ys, 0.0, 0.0,
        0.0, 0.0, c, 1.0,
        0.0, 0.0, d, 0.0,
    ];
    #[rustfmt::skip]
    let clip_to_view = [
        1.0 / xs, 0.0, 0.0, 0.0,
        0.0, 1.0 / ys, 0.0, 0.0,
        0.0, 0.0, 0.0, 1.0 / d,
        0.0, 0.0, 1.0, -c / d,
    ];
    (view_to_clip, clip_to_view)
}

fn constants(l: &Inputs) -> Constants {
    let (w, h) = (l.render[0].max(1) as f32, l.render[1].max(1) as f32);
    let (view_to_clip, clip_to_view) = projection(l.render, l.fov, l.near, l.far, l.inverted);
    #[rustfmt::skip]
    let identity = [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ];
    let flag = |b: bool| if b { TRUE } else { FALSE };
    Constants {
        base: Base::new(CONSTANTS, 2),
        camera_view_to_clip: view_to_clip,
        clip_to_camera_view: clip_to_view,
        clip_to_lens_clip: identity,
        clip_to_prev_clip: identity,
        prev_clip_to_clip: identity,
        jitter_offset: l.jitter,
        mvec_scale: l.mvec_scale,
        camera_pinhole_offset: [0.0; 2],
        camera_pos: [0.0; 3],
        camera_up: [0.0, 1.0, 0.0],
        camera_right: [1.0, 0.0, 0.0],
        camera_fwd: [0.0, 0.0, 1.0],
        camera_near: l.near,
        camera_far: l.far,
        camera_fov: l.fov,
        camera_aspect_ratio: w / h,
        motion_vectors_invalid_value: f32::MAX,
        depth_inverted: flag(l.inverted),
        camera_motion_included: TRUE,
        motion_vectors_3d: FALSE,
        reset: flag(l.reset),
        orthographic_projection: FALSE,
        motion_vectors_dilated: FALSE,
        motion_vectors_jittered: TRUE,
        min_relative_linear_depth_object_separation: 40.0,
    }
}

/// Presents in a row without a new upscale after which frame generation is
/// switched off (menus, loading, videos), until the next upscale.
const STALE_MOST: u32 = 3;

/// Right before the proxy's `Present` of title frame `frame`, of a back
/// buffer of `back` size: DLSS-G on or off, the latency markers, and for a
/// frame with an upscale its constants and tags: depth, motion vectors, the
/// colour without the interface (`hudless`, the scene plane the back buffer
/// is a copy of, in the state it is in at the present) and the interface
/// (`ui`, Series S's second plane, premultiplied, when it is the back
/// buffer's size). `VICTORY_FRAMEGEN_HUDLESS=0`: neither of those two.
pub(crate) fn present_start(
    frame: u64,
    with: Option<&ForPresent>,
    back: [u32; 2],
    hudless: Option<(&ID3D12Resource, u32)>,
    ui: Option<(&ID3D12Resource, u32)>,
) {
    let Some(fg) = fg() else { return };
    let mut fg = fg.lock().unwrap_or_else(|e| e.into_inner());
    let index = frame as u32;
    let inputs = with.and_then(|w| w.inputs.clone());
    let fresh = with.is_some_and(|w| w.fresh);
    fg.stale = if fresh { 0 } else { fg.stale.saturating_add(1) };
    let on = inputs.is_some() && fg.stale < STALE_MOST;
    if fg.on != Some(on) {
        let options = DlssgOptions::new(on, fg.frames);
        let viewport = Viewport::new(VIEWPORT);
        // SAFETY: structures that outlive the call.
        let r = unsafe { (fg.set_options)(&viewport, &options) };
        tracing::info!(frame, on, shown_per_rendered = fg.frames + 1, result = result_name(r), "framegen: DLSS-G switched");
        fg.on = Some(on);
    }
    let Some(token) = new_token(&fg, index) else { return };
    for marker in [SIMULATION_START, SIMULATION_END, RENDER_SUBMIT_START, RENDER_SUBMIT_END] {
        mark(&fg, marker, token);
    }
    if let Some(l) = inputs.as_deref() {
        let viewport = Viewport::new(VIEWPORT);
        let constants = constants(l);
        // SAFETY: structures that outlive the call.
        let r = unsafe { (fg.sl.set_constants)(&constants, token, &viewport) };
        let extent = Extent([0, 0, l.render[0], l.render[1]]);
        let mut resources = vec![
            (BUFFER_DEPTH, sl_resource(&l.depth.texture, l.depth.state.load(Ordering::Relaxed)), extent),
            (BUFFER_MOTION_VECTORS, sl_resource(&l.motion.texture, l.motion.state.load(Ordering::Relaxed)), extent),
        ];
        let size = |t: &ID3D12Resource| {
            // SAFETY: a plain query.
            let d = unsafe { t.GetDesc() };
            [d.Width as u32, d.Height]
        };
        if env_on("VICTORY_FRAMEGEN_HUDLESS", true) {
            if let Some((t, state)) = hudless.filter(|(t, _)| size(t) == back) {
                resources.push((BUFFER_HUDLESS_COLOR, sl_resource(t, state), Extent::default()));
            }
            if let Some((t, state)) = ui.filter(|(t, _)| size(t) == back) {
                resources.push((BUFFER_UI_COLOR_AND_ALPHA, sl_resource(t, state), Extent::default()));
            }
        }
        let tags: Vec<ResourceTag> = resources
            .iter_mut()
            .map(|(kind, resource, extent)| ResourceTag {
                base: Base::new(RESOURCE_TAG, 1),
                resource,
                kind: *kind,
                lifecycle: VALID_UNTIL_PRESENT,
                extent: *extent,
            })
            .collect();
        // SAFETY: live textures (held by `l` and the presenter through the
        // present), in the states they are in at the present; no list is
        // needed for tags valid until the present.
        let t = unsafe {
            (fg.sl.set_tag_for_frame)(token, &viewport, tags.as_ptr(), tags.len() as u32, std::ptr::null_mut())
        };
        static FIRST: std::sync::Once = std::sync::Once::new();
        FIRST.call_once(|| {
            let kinds: Vec<u32> = tags.iter().map(|t| t.kind).collect();
            tracing::info!(
                frame,
                ?kinds,
                constants = result_name(r),
                tags = result_name(t),
                "framegen: first present with constants and tags (0 depth, 1 motion vectors, 2 colour without the interface, 23 interface)"
            );
        });
        if (r != 0 || t != 0) && fg.told < 5 {
            fg.told += 1;
            tracing::warn!(frame, constants = result_name(r), tags = result_name(t), "framegen: constants or tags refused");
        }
    }
    mark(&fg, PRESENT_START, token);
}

/// Right after the proxy's `Present` of title frame `frame`: the latency
/// marker, now and then what DLSS-G says of itself, and Reflex's pacing of
/// the next frame (`VICTORY_FRAMEGEN_REFLEX=0`: not).
pub(crate) fn present_end(frame: u64) {
    let Some(fg) = fg() else { return };
    let mut fg = fg.lock().unwrap_or_else(|e| e.into_inner());
    let index = frame as u32;
    if let Some(token) = new_token(&fg, index) {
        mark(&fg, PRESENT_END, token);
    }
    static PRESENTS: AtomicU32 = AtomicU32::new(0);
    let n = PRESENTS.fetch_add(1, Ordering::Relaxed) + 1;
    if n.is_multiple_of(30) {
        let mut state = DlssgState::new();
        let viewport = Viewport::new(VIEWPORT);
        // SAFETY: structures that outlive the call.
        let r = unsafe { (fg.get_state)(&viewport, &mut state, std::ptr::null()) };
        let changed = r == 0 && state.status != fg.status;
        if changed || n.is_multiple_of(600) || (n / 30).is_power_of_two() {
            tracing::info!(
                frame,
                on = ?fg.on,
                status = format_args!("{:#x} (0 working; 1 resolution too low, 2 Reflex missing, 4 HDR format, 8 constants invalid, 0x10 back buffer index)", state.status),
                shown_last_present = state.frames_presented,
                most = state.frames_to_generate_max + 1,
                vram_mb = state.estimated_vram >> 20,
                result = result_name(r),
                "framegen: DLSS-G state"
            );
        }
        if r == 0 {
            fg.status = state.status;
        }
    }
    if let Some(sleep) = fg.reflex_sleep
        && let Some(token) = new_token(&fg, index.wrapping_add(1))
    {
        // SAFETY: a token of Streamline's.
        let _ = unsafe { sleep(token) };
    }
}

/// Before the swap chain's buffers are made again: DLSS-G off (Streamline
/// asks for it around a resize), on again at the next present.
pub(crate) fn before_resize() {
    let Some(fg) = fg() else { return };
    let mut fg = fg.lock().unwrap_or_else(|e| e.into_inner());
    let options = DlssgOptions::new(false, fg.frames);
    let viewport = Viewport::new(VIEWPORT);
    // SAFETY: structures that outlive the call.
    let _ = unsafe { (fg.set_options)(&viewport, &options) };
    fg.on = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_without_dlss_g_names_what_is_missing() {
        let dir = std::env::temp_dir().join(format!("victory-framegen-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // The DLSS folder this tree shipped with: DLSS super resolution only.
        for f in ["nvngx_dlss.dll", "sl.common.dll", "sl.dlss.dll", "sl.interposer.dll"] {
            std::fs::write(dir.join(f), b"").unwrap();
        }
        assert_eq!(
            missing_files(&dir),
            ["sl.dlss_g.dll", "nvngx_dlssg.dll", "sl.reflex.dll", "sl.pcl.dll"]
        );
        for f in FILES {
            std::fs::write(dir.join(f), b"").unwrap();
        }
        assert!(missing_files(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn typeless_depth_and_motion_are_read_as_floats() {
        assert_eq!(typed(39), 41);
        assert_eq!(typed(19), 21);
        assert_eq!(typed(33), 34);
        assert_eq!(typed(87), 87);
    }

    #[test]
    fn the_projection_and_its_inverse_agree() {
        let row = |v: [f32; 4], m: &[f32; 16]| {
            let mut out = [0.0f32; 4];
            for (c, o) in out.iter_mut().enumerate() {
                *o = (0..4).map(|r| v[r] * m[r * 4 + c]).sum();
            }
            out
        };
        for inverted in [false, true] {
            let (to_clip, to_view) = projection([960, 540], 1.0, 0.15, 10000.0, inverted);
            // Depth is 0 at the near plane and 1 at the far one, the other
            // way round inverted.
            let depth = |z: f32| {
                let c = row([0.0, 0.0, z, 1.0], &to_clip);
                c[2] / c[3]
            };
            let (at_near, at_far) = (depth(0.15), depth(10000.0));
            let (near_should, far_should) = if inverted { (1.0, 0.0) } else { (0.0, 1.0) };
            assert!((at_near - near_should).abs() < 1e-3, "{inverted} {at_near}");
            assert!((at_far - far_should).abs() < 1e-3, "{inverted} {at_far}");
            // And back.
            let p = [1.5, -2.0, 25.0, 1.0];
            let v = row(row(p, &to_clip), &to_view);
            for i in 0..3 {
                assert!((v[i] / v[3] - p[i]).abs() < 1e-2, "{inverted} {v:?}");
            }
        }
    }

    /// Streamline started with frame generation's features on the default
    /// adapter, its answer for DLSS-G logged, and a factory upgraded to its
    /// proxy when it runs. Needs `VICTORY_FRAMEGEN=1` and Streamline's
    /// folder (`VICTORY_FRAMEGEN_DIR`); skipped otherwise. Run it alone
    /// (Streamline starts once a process): `cargo test -p victory-gpu --lib
    /// streamline_answers -- --nocapture`.
    #[test]
    fn streamline_answers_for_dlss_g_on_this_gpu() {
        if !wanted() || dir().is_none() {
            eprintln!("VICTORY_FRAMEGEN=1 and VICTORY_FRAMEGEN_DIR unset; skipped");
            return;
        }
        let _ = tracing_subscriber_for_tests();
        let mut device: Option<ID3D12Device> = None;
        // SAFETY: the default adapter.
        unsafe {
            windows::Win32::Graphics::Direct3D12::D3D12CreateDevice(
                None,
                windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_12_0,
                &mut device,
            )
        }
        .expect("device");
        let device = device.expect("device");
        // SAFETY: plain object creation.
        let factory: IDXGIFactory2 = unsafe {
            windows::Win32::Graphics::Dxgi::CreateDXGIFactory2(windows::Win32::Graphics::Dxgi::DXGI_CREATE_FACTORY_FLAGS(0))
        }
        .expect("factory");
        let native = factory.as_raw();
        let given = start(&device, factory);
        eprintln!(
            "frame generation running: {}, factory upgraded to Streamline's proxy: {}",
            active(),
            given.as_raw() != native
        );
    }

    /// Prints the log to the test's output (`--nocapture`), if a subscriber
    /// can be set.
    fn tracing_subscriber_for_tests() -> Option<()> {
        struct Print;
        impl tracing::Subscriber for Print {
            fn enabled(&self, m: &tracing::Metadata<'_>) -> bool {
                *m.level() <= tracing::Level::INFO
            }
            fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
                tracing::span::Id::from_u64(1)
            }
            fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
            fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
            fn event(&self, e: &tracing::Event<'_>) {
                struct Fields(String);
                impl tracing::field::Visit for Fields {
                    fn record_debug(&mut self, f: &tracing::field::Field, v: &dyn std::fmt::Debug) {
                        self.0 += &format!(" {}={v:?}", f.name());
                    }
                }
                let mut fields = Fields(String::new());
                e.record(&mut fields);
                eprintln!("{} {}", e.metadata().level(), fields.0);
            }
            fn enter(&self, _: &tracing::span::Id) {}
            fn exit(&self, _: &tracing::span::Id) {}
        }
        tracing::subscriber::set_global_default(Print).ok()
    }

    #[test]
    fn options_and_state_have_streamline_s_versions() {
        let o = DlssgOptions::new(true, 3);
        assert_eq!((o.mode, o.frames_to_generate, o.base.version), (DLSSG_ON, 3, 5));
        assert_eq!(DlssgOptions::new(false, 0).frames_to_generate, 1);
        assert_eq!(DlssgState::new().base.version, 4);
    }
}
