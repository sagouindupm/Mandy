//! Mesh draws launched by the card: an amplification shader of ours
//! (`launch.hlsl`, compiled by the build: [`CODE`]) in front of every
//! translated mesh shader (`VICTORY_MESH_LAUNCH`, on by default; `0`: every
//! indirect mesh record clamped by a pass of `crate::indirect` before its
//! draw, as before).
//!
//! Ported from stellarr-studio/kelvaroq patch 0041 (Stellarr Studio,
//! GPL-3.0-or-later) and fitted to the batched clamps of this tree.
//!
//! The title draws its meshes one indirect record a call, each from the
//! next slot of an argument buffer its own compute passes wrote: some 3,500
//! mesh records a frame of gameplay. Each was bounded by a compute pass of
//! ours between two waits for every earlier shader write: one pass a record
//! before, one pass for the records of a run of draws with the batches of
//! `crate::indirect::batch` (`VICTORY_INDIRECT_BATCH`), whose run ends at
//! every dispatch and every draw that stores to buffers, so the card still
//! went from drawing to compute and back, emptying itself twice, many
//! times a frame. The fork measured, with one pass a record (RTX 5080,
//! prologue): heavy frames 198 -> 120 ms, the card busy 78% -> 35%.
//!
//! Here the record is read by the draw itself: the pipeline's amplification
//! stage loads its three words where the title's passes wrote them, bounds
//! them (65,535 groups, one-dimensional), charges the list's budget (a
//! record that does not fit runs empty and takes the rest), and launches
//! the mesh groups. The title's own
//! barrier (a wait for every shader write, `crate::list`) orders its passes
//! before the draw, as on the console; nothing of ours runs between draws.
//! A mesh dispatch the CPU knows (`DispatchMesh`) gives its count as a root
//! constant and is launched the same way, unbounded by the budget as
//! before.
//!
//! The draws' amplification groups may run in another order than the
//! list's (the card overlaps draws; nothing orders them now), so when the
//! list's budget runs out, which of its records run empty is the card's
//! order and no longer the list's: the bound on the list's total work, the
//! guard against a list that runs for seconds, is the same.
//!
//! An indirect call with a count buffer or more than one record keeps the
//! clamp and runs on the same pipeline made without the amplification
//! stage (`HostPipeline::launch`'s `plain`, made the first time one needs
//! it): the translated mesh shader declares the payload either way
//! (`victory_shader::emit::MESH_PAYLOAD_BYTES`), which the driver takes
//! with or without an amplification shader.

use std::sync::atomic::{AtomicU64, Ordering};

use windows::Win32::Graphics::Direct3D12::*;

use crate::host::HostList;

/// The amplification shader's code: `launch.hlsl` compiled by the build
/// (`build.rs`) with the Windows SDK's dxc, signed by its `dxil.dll`:
///
/// ```text
/// dxc -nologo -T as_6_6 -E main -Fo launch.dxil launch.hlsl
/// ```
///
/// No compiled bytes are kept in the tree (`docs/asset-policy.md`). Empty
/// where the build found no compiler: then there is no stage ([`on`]).
const CODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/launch.dxil"));

/// The root constant that has the card read the record.
const DIRECT_NONE: u32 = u32::MAX;

/// Where the amplification shader binds: `b0`, `t0`, `u0` in this space.
const SPACE: u32 = victory_shader::emit::LAUNCH_SPACE;

/// Whether mesh pipelines get the amplification stage and indirect mesh
/// records are launched by it (`VICTORY_MESH_LAUNCH=0`: not; on by
/// default, as the fork has it: measured there). Never without the stage's
/// code (a build without the Windows SDK's dxc).
pub(crate) fn on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        let asked = std::env::var_os("VICTORY_MESH_LAUNCH").is_none_or(|v| v != "0");
        if asked && CODE.is_empty() {
            tracing::warn!("VICTORY_MESH_LAUNCH: this build has no launch stage (no dxc when it was built): mesh records are clamped");
        }
        asked && !CODE.is_empty()
    })
}

/// The amplification stage's code.
pub(crate) fn code() -> D3D12_SHADER_BYTECODE {
    D3D12_SHADER_BYTECODE { pShaderBytecode: CODE.as_ptr().cast(), BytecodeLength: CODE.len() }
}

/// The bytes of [`code`], for the pipeline library's names.
pub(crate) fn bytes() -> &'static [u8] {
    CODE
}

/// The three root parameters the stage reads, in order: the direct group
/// count (one constant), the record (a root SRV), the list's budget (a
/// root UAV). Five root words.
pub(crate) fn params() -> [D3D12_ROOT_PARAMETER1; 3] {
    let descriptor = |kind| D3D12_ROOT_PARAMETER1 {
        ParameterType: kind,
        Anonymous: D3D12_ROOT_PARAMETER1_0 {
            Descriptor: D3D12_ROOT_DESCRIPTOR1 {
                ShaderRegister: 0,
                RegisterSpace: SPACE,
                Flags: D3D12_ROOT_DESCRIPTOR_FLAG_DATA_VOLATILE,
            },
        },
        ShaderVisibility: D3D12_SHADER_VISIBILITY_AMPLIFICATION,
    };
    [
        D3D12_ROOT_PARAMETER1 {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
            Anonymous: D3D12_ROOT_PARAMETER1_0 {
                Constants: D3D12_ROOT_CONSTANTS { ShaderRegister: 0, RegisterSpace: SPACE, Num32BitValues: 1 },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_AMPLIFICATION,
        },
        descriptor(D3D12_ROOT_PARAMETER_TYPE_SRV),
        descriptor(D3D12_ROOT_PARAMETER_TYPE_UAV),
    ]
}

/// Mesh draws launched since the last line: records read by the card, and
/// dispatches whose count the CPU gave.
static LAUNCHED: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];

/// What one launch reads.
#[derive(Clone, Copy)]
pub(crate) enum Launch {
    /// The record at this address on the card (4-byte aligned).
    Record(u64),
    /// A group count known on the CPU (bounded there).
    Direct(u32),
}

/// Records the launch of one mesh draw in `list`, which has the mesh
/// pipeline with its stage bound and everything else the draw needs; the
/// stage's parameters are at `roots`. False, with nothing recorded, if the
/// list has no budget for a record.
pub(crate) fn record(list: &HostList, roots: [u32; 3], what: Launch) -> bool {
    let Ok(l6) = windows::core::Interface::cast::<ID3D12GraphicsCommandList6>(&list.list) else {
        return false;
    };
    let budget = list.budget();
    let (direct, record, budget) = match what {
        Launch::Record(address) => {
            let Some(budget) = budget else {
                return false;
            };
            (DIRECT_NONE, address, budget)
        }
        // The stage reads neither the record nor the budget; live
        // addresses all the same where the list has them.
        Launch::Direct(x) => (x.min(65_535), budget.unwrap_or(0), budget.unwrap_or(0)),
    };
    launch(&list.list, &l6, roots, (direct, record, budget));
    LAUNCHED[usize::from(matches!(what, Launch::Direct(_)))].fetch_add(1, Ordering::Relaxed);
    true
}

/// Binds the stage's root parameters and launches its one group: `direct`
/// (a group count, or [`DIRECT_NONE`]), the record's and the budget's
/// addresses.
fn launch(
    list: &ID3D12GraphicsCommandList,
    l6: &ID3D12GraphicsCommandList6,
    roots: [u32; 3],
    (direct, record, budget): (u32, u64, u64),
) {
    // SAFETY: an open list with the mesh pipeline and its root signature
    // set; the record and the budget are live buffers the list keeps.
    unsafe {
        list.SetGraphicsRoot32BitConstant(roots[0], direct, 0);
        list.SetGraphicsRootShaderResourceView(roots[1], record);
        list.SetGraphicsRootUnorderedAccessView(roots[2], budget);
        l6.DispatchMesh(1, 1, 1);
    }
}

/// For the frame-rate line.
pub(crate) fn line(frames: f64) {
    let [records, direct] = [0, 1].map(|i| LAUNCHED[i].swap(0, Ordering::Relaxed));
    if records + direct > 0 {
        tracing::info!(
            "mesh draws launched by the card per frame: {:.0} records read where the title wrote them, {:.0} counts \
             known on the CPU (no pass of ours between them; VICTORY_MESH_LAUNCH=0: the records clamped by passes)",
            records as f64 / frames,
            direct as f64 / frames
        );
    }
}

