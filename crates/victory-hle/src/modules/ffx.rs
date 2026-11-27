//! `amd_fidelityfx_loader_xbox.dll` and `amd_fidelityfx_upscaler_xbox.dll`:
//! the title's FidelityFX upscaler.
//!
//! Without `VICTORY_FFX_DIR` or `VICTORY_DLSS_DIR` both are the title's own
//! DLLs and run as they are. With `VICTORY_FFX_DIR` they are served by AMD's
//! PC upscaler on the host GPU (`victory_gpu::ffx`, [`module`]); with
//! `VICTORY_DLSS_DIR` they are answered by NVIDIA DLSS through Streamline
//! instead.

#[cfg(windows)]
use crate::{HleExport, HleModule};

/// Whether DLSS answers the title's FidelityFX calls
/// (`victory_gpu::ffx::dlss_enabled`).
#[cfg(windows)]
pub fn dlss() -> bool {
    victory_gpu::ffx::dlss_enabled()
}

/// Whether frame generation is asked for (`VICTORY_FRAMEGEN=1`,
/// `victory_gpu::ffx::framegen`).
#[cfg(windows)]
pub fn framegen() -> bool {
    victory_gpu::ffx::framegen::wanted()
}

/// Frame generation's stand-in for the title's own upscaler's dispatch
/// method, and where it is told the method's address
/// (`victory_loader::tap_upscale`).
#[cfg(windows)]
pub fn framegen_upscale_tap() -> (usize, fn(usize)) {
    use victory_gpu::ffx::framegen;
    (
        framegen::own_upscale as *const () as usize,
        framegen::set_own_upscale_original,
    )
}

/// The render scale the title is set to for DLSS's `VICTORY_DLSS_MODE` in
/// the Series S mode (`victory_gpu::ffx::dlss_render_scale`); `None` leaves
/// its own.
#[cfg(windows)]
pub fn dlss_render_scale(series_s: bool) -> Option<f32> {
    victory_gpu::ffx::dlss_render_scale(series_s)
}

/// With `VICTORY_FFX_DIR`: the five FidelityFX calls of both DLLs, served by
/// AMD's PC upscaler instead of the console DLLs.
#[cfg(windows)]
pub(crate) fn module() -> HleModule {
    use victory_gpu::ffx;
    let export = |name, address: usize| HleExport { name, address };
    HleModule {
        name: "ffx",
        dlls: &[
            "amd_fidelityfx_loader_xbox.dll",
            "amd_fidelityfx_upscaler_xbox.dll",
        ],
        exports: vec![
            export(
                "ffxCreateContext",
                ffx::ffxCreateContext as *const () as usize,
            ),
            export(
                "ffxDestroyContext",
                ffx::ffxDestroyContext as *const () as usize,
            ),
            export("ffxConfigure", ffx::ffxConfigure as *const () as usize),
            export("ffxQuery", ffx::ffxQuery as *const () as usize),
            export("ffxDispatch", ffx::ffxDispatch as *const () as usize),
        ],
    }
}

/// DLSS beside the title's own FidelityFX DLLs: the executable's imports of
/// them that DLSS takes (`victory_gpu::ffx::bridge`).
#[cfg(windows)]
pub use victory_gpu::ffx::bridge;
