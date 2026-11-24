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
