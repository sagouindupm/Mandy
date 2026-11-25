//! A console wave64 compute program on a host of 32-lane waves: a wave reduction inside a
//! per-lane branch, with that branch taken wave-wide ([`crate::structure::structure_wide`])
//! so that the two host waves can exchange their halves inside it. Lowered, validated by
//! `dxil.dll` when present, and run on the host GPU when it takes waves of 32.

use std::collections::HashSet;

use super::host::{Dispatch, Host, Root, View};
use super::{HostShader, lower, validator};
use crate::native::{
    ComputeIo, HwStage, NativeProgram, ResourceType, RootParam, RootType, StageIo, UsedResource,
};
use crate::{ShaderError, io, ir, resource, structure};

const S_ENDPGM: u32 = 0xBF81_0000;

/// 64 threads in one console wave. Each thread t holds the bit `1 << (t >> 1)`; the threads
/// below s0 (a root constant) OR theirs over the wave inside `if (t < s0) { ... }` and store
/// the result to dword 0 of the raw UAV.
/// `skip`: the branch past the body when no thread is below s0: `s_cbranch_execz`, or
/// `s_cbranch_scc0` on the SCC `s_and_saveexec` sets ("some lane is left").
fn program(skip: u32) -> NativeProgram {
    native(&words(skip))
}

/// [`program`]'s code.
fn words(skip: u32) -> Vec<u32> {
    let mut words = vec![
        0x7E0A_0281, // v_mov_b32 v5, 1
        0x2C0C_0081, // v_lshrrev_b32 v6, 1, v0
        0x340E_0B06, // v_lshlrev_b32 v7, v6, v5
        0x7D88_0000, // v_cmp_gt_u32 vcc, s0, v0
        0xBE8C_246A, // s_and_saveexec_b64 s[12:13], vcc
        skip | 23,   // the branch to the restore of EXEC
    ];
    words.extend_from_slice(&super::wave::WAVE_OR_V7_TO_S7);
    words.extend_from_slice(&[
        0x7E00_0207, // v_mov_b32 v0, s7
        0x38 << 26 | 0x1C << 18,
        0x80 << 24 | 2 << 16, // buffer_store_dword v0, off, s[8:11], 0
        0xBEFE_040C, // s_mov_b64 exec, s[12:13]
    ]);
    words
}

/// A wave64 compute program of 64 threads (one console wave; the thread index in v0) with 4
/// root constants in s0..s3 and a raw UAV's descriptor in s[8:11], then `body`.
fn native(body: &[u32]) -> NativeProgram {
    native_of(body, true, 64)
}

/// The same for a console of 32-lane waves when `wave64` is not set, with `threads` threads.
fn native_of(body: &[u32], wave64: bool, threads: u32) -> NativeProgram {
    let mut words = vec![
        0xBE85_0380, // s_mov_b32 s5, 0
        0x3D << 26 | 0x02 << 18 | 8 << 6 | 2,
        125 << 25, // s_load_dwordx4 s[8:11], s[4:5], 0
    ];
    words.extend_from_slice(body);
    words.push(S_ENDPGM);
    NativeProgram {
        stage: HwStage::Cs,
        wave64,
        vgprs: 16,
        layout: vec![
            RootParam {
                root: RootType::Constants,
                table_offset_dwords: 0,
                sgprs: 4,
                base_register: 0,
                user_data: 0,
                slot: 0,
            },
            RootParam {
                root: RootType::Table,
                table_offset_dwords: 0,
                sgprs: 1,
                base_register: 0,
                user_data: 4,
                slot: 1,
            },
        ],
        used: vec![UsedResource {
            kind: ResourceType::Uav,
            root: RootType::Table,
            table_offset_dwords: 0,
            lower_bound: 0,
            stride: 0,
            dwords: 4,
            user_data: 4,
            slot: 1,
        }],
        io: StageIo::Cs(ComputeIo {
            rsrc2: 5 << 1, // 5 user SGPRs: s0..s4
            threads: [threads, 1, 1],
        }),
        code: words.iter().flat_map(|w| w.to_le_bytes()).collect(),
    }
}

/// `program()` for a host of 32-lane waves, with the branches closing `wide` taken wave-wide.
fn lowered(native: &NativeProgram, p: &ir::Program, wide: &HashSet<usize>) -> Result<HostShader, ShaderError> {
    lowered_on(native, p, wide, 32)
}

/// [`lowered`] for a host whose waves have at most `host_wave` lanes.
fn lowered_on(
    native: &NativeProgram,
    p: &ir::Program,
    wide: &HashSet<usize>,
    host_wave: u32,
) -> Result<HostShader, ShaderError> {
    let st = structure::structure_wide(p, wide).expect("structure");
    let res = resource::track(p, &native.layout, native.stage, Some(&st)).expect("resource::track");
    let io = io::raise(p, native).expect("io::raise");
    let was = lower::TEST_HOST_WAVE.replace(Some(host_wave));
    let lowered = lower::lower(&lower::Parts {
        native,
        p,
        st: &st,
        io: &io,
        res: &res,
        inputs: Vec::new(),
        outputs: Vec::new(),
        prims: Vec::new(),
        mesh_stride: 0,
    });
    lower::TEST_HOST_WAVE.set(was);
    let lowered = lowered?;
    Ok(HostShader {
        stage: HwStage::Cs,
        dxil: lowered.dxil,
        user_data: lowered.user_data,
        samplers: lowered.samplers,
        static_samplers: Vec::new(),
        root_cbvs: lowered.root_cbvs,
        root_srvs: lowered.root_srvs,
        root_uavs: lowered.root_uavs,
        root_indexed_views: res.root_indexed_views,
    })
}

#[test]
fn a_sum_of_halves_over_the_wave_is_the_consoles_tree_bit_for_bit() {
    // thread t holds the 16-bit float with bits 0x3C00 + 37 t (1.0 and up, with odd
    // mantissas, so that additions round); all of them store the wave's sum
    let mut words = vec![
        0x340C_0085, // v_lshlrev_b32 v6, 5, v0
        0x340E_0082, // v_lshlrev_b32 v7, 2, v0
        0x4A0E_0F06, // v_add_nc_u32 v7, v6, v7
        0x4A0E_0F00, // v_add_nc_u32 v7, v0, v7
        0x4A0E_0EFF,
        0x0000_3C00, // v_add_nc_u32 v7, 0x3c00, v7
    ];
    // (the wave OR's words with v_add_f16 for v_or_b32: opcode 50 for 28)
    words.extend(super::wave::WAVE_OR_V7_TO_S7.iter().map(|&w| if w >> 25 == 28 { w & 0x01FF_FFFF | 50 << 25 } else { w }));
    words.extend_from_slice(&[
        0x7E00_0207, // v_mov_b32 v0, s7
        0x38 << 26 | 0x1C << 18,
        0x80 << 24 | 2 << 16, // buffer_store_dword v0, off, s[8:11], 0
    ]);
    let native = native(&words);
    let p = ir::build(&native.code, native.wave64, native.vgprs).expect("ir::build");
    assert_eq!(p.blocks.iter().flat_map(|b| &b.insts).filter(|i| i.name() == "v_add_f16").count(), 6);
    let shader = lowered(&native, &p, &HashSet::new()).expect("lower");
    if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
        let mut copy = shader.dxil.clone();
        v.validate(&mut copy).expect("DXIL validation failed");
    }
    // the console's sum: lanes 2k and 2k+1, then pairs, ... each addition rounded to 16 bits
    // (to nearest, ties to even); values here are normal numbers
    let wide = |h: u16| f32::from_bits((u32::from(h >> 10) + 112) << 23 | u32::from(h & 0x3FF) << 13);
    let narrow = |f: f32| {
        let b = f.to_bits();
        let h = ((b >> 23) - 112) << 10 | (b >> 13 & 0x3FF);
        let rest = b & 0x1FFF;
        (h + u32::from(rest > 0x1000 || rest == 0x1000 && h & 1 == 1)) as u16
    };
    let add = |a: u16, b: u16| narrow(wide(a) + wide(b));
    let terms: Vec<u16> = (0..64u16).map(|t| 0x3C00 + 37 * t).collect();
    let mut tree = terms.clone();
    while tree.len() > 1 {
        tree = tree.chunks(2).map(|c| add(c[0], c[1])).collect();
    }
    let in_a_row = terms.iter().copied().reduce(add).unwrap();
    assert_ne!(tree[0], in_a_row, "these terms tell the tree from another order");
    let _one = super::valu_tests::ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(host) = Host::new(false) else {
        eprintln!("no SM 6.6 hardware device; not run");
        return;
    };
    let (root, pso) = match host.compute_pipeline(&shader) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("the hardware device does not take this program (waves of 32?): {e}; not run");
            return;
        }
    };
    let ud = vec![0u32; shader.user_data_dwords().max(1) as usize];
    let out = host
        .dispatch(
            &root,
            &pso,
            &Dispatch {
                roots: vec![(0, Root::Constants(ud))],
                views: &[View::RawUav(1)],
                samplers: &[],
                groups: [1, 1, 1],
            },
        )
        .unwrap_or_else(|e| panic!("dispatch failed: {e}"));
    // (the high half is lane 0's term's: zero)
    assert_eq!(out[0][0], u32::from(tree[0]), "tree {:#x}, in a row {in_a_row:#x}", tree[0]);
    eprintln!("ran on the hardware device: the sum {:#x} of the console's tree", tree[0]);
}

/// The console's 16-bit wave sum where the lanes at or past s0 have left the program (a
/// thread past the end of the data): on the console they add +0, and after each step of the
/// tree they hold their run's partial sum all the same. On a host of 64-lane waves (AMD) the
/// lanes that are here read each partner run's sum from a lane of it that is here.
#[test]
fn a_sum_of_halves_where_lanes_have_left_is_the_consoles_tree_bit_for_bit() {
    let mut words = vec![
        0x7DA8_0000, // v_cmpx_gt_u32 exec, s0, v0: the lanes below s0 stay
        0, // s_cbranch_execz to the end (filled in below)
        0x340C_0085, // v_lshlrev_b32 v6, 5, v0
        0x340E_0082, // v_lshlrev_b32 v7, 2, v0
        0x4A0E_0F06, // v_add_nc_u32 v7, v6, v7
        0x4A0E_0F00, // v_add_nc_u32 v7, v0, v7
        0x4A0E_0EFF,
        0x0000_3C00, // v_add_nc_u32 v7, 0x3c00, v7
    ];
    words.extend(super::wave::WAVE_OR_V7_TO_S7.iter().map(|&w| if w >> 25 == 28 { w & 0x01FF_FFFF | 50 << 25 } else { w }));
    words.extend_from_slice(&[
        0x7E00_0207, // v_mov_b32 v0, s7
        0x38 << 26 | 0x1C << 18,
        0x80 << 24 | 2 << 16, // buffer_store_dword v0, off, s[8:11], 0
    ]);
    // past every word after it, to the program's end
    words[1] = 0xBF88_0000 | (words.len() as u32 - 2);
    let native = native(&words);
    let p = ir::build(&native.code, native.wave64, native.vgprs).expect("ir::build");
    let shader = lowered_on(&native, &p, &HashSet::new(), 64).expect("lower");
    if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
        let mut copy = shader.dxil.clone();
        v.validate(&mut copy).expect("DXIL validation failed");
    }
    let wide = |h: u16| {
        if h == 0 {
            0.0
        } else {
            f32::from_bits((u32::from(h >> 10) + 112) << 23 | u32::from(h & 0x3FF) << 13)
        }
    };
    let narrow = |f: f32| {
        if f == 0.0 {
            return 0;
        }
        let b = f.to_bits();
        let h = ((b >> 23) - 112) << 10 | (b >> 13 & 0x3FF);
        let rest = b & 0x1FFF;
        (h + u32::from(rest > 0x1000 || rest == 0x1000 && h & 1 == 1)) as u16
    };
    let add = |a: u16, b: u16| narrow(wide(a) + wide(b));
    let _one = super::valu_tests::ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let mut ran = false;
    for stay in [64u32, 63, 40, 33, 32, 17, 5, 1] {
        let terms: Vec<u16> = (0..64u32)
            .map(|t| if t < stay { (0x3C00 + 37 * t) as u16 } else { 0 })
            .collect();
        let mut tree = terms.clone();
        while tree.len() > 1 {
            tree = tree.chunks(2).map(|c| add(c[0], c[1])).collect();
        }
        let Some(got) = on_the_card(&shader, [stay, 0]) else {
            return;
        };
        ran = true;
        // (the high half is lane 0's term's: zero)
        assert_eq!(got, u32::from(tree[0]), "{stay} lanes stay: got {got:#x}, the console's tree {:#x}", tree[0]);
    }
    if ran {
        eprintln!("ran on the hardware device: the console's tree with lanes gone, in every case");
    }
}

/// One `v_add_f16` whose exact result lies between two halves: 1 + 3/1024 (0x3C03) plus 2
/// (0x4000) is 3 + 1.5 steps of that range, which the console rounds up to 0x4202. The
/// host's `f32tof16` may give 0x4201 (toward zero); with `VICTORY_F16_NEAREST=1` the
/// translation gives the console's whatever the host does.
#[test]
fn a_16_bit_sum_rounds_to_nearest_when_asked_to() {
    let words = [
        0x7E0E_02FF,
        0x0000_3C03, // v_mov_b32 v7, 0x3c03
        0x7E0C_02FF,
        0x0000_4000, // v_mov_b32 v6, 0x4000
        0x6400_0D07, // v_add_f16 v0, v7, v6
        0x38 << 26 | 0x1C << 18,
        0x80 << 24 | 2 << 16, // buffer_store_dword v0, off, s[8:11], 0
    ];
    let native = native(&words);
    let p = ir::build(&native.code, native.wave64, native.vgprs).expect("ir::build");
    assert!(p.blocks.iter().flat_map(|b| &b.insts).any(|i| i.name() == "v_add_f16"));
    let _one = super::valu_tests::ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    for nearest in [false, true] {
        let was = lower::TEST_F16_NEAREST.replace(Some(nearest));
        let shader = lowered(&native, &p, &HashSet::new());
        lower::TEST_F16_NEAREST.set(was);
        let shader = shader.expect("lower");
        if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
            let mut copy = shader.dxil.clone();
            v.validate(&mut copy).expect("DXIL validation failed");
        }
        let Ok(host) = Host::new(false) else {
            eprintln!("no SM 6.6 hardware device; not run");
            return;
        };
        let Ok((root, pso)) = host.compute_pipeline(&shader) else {
            eprintln!("the hardware device does not take this program; not run");
            return;
        };
        let ud = vec![0u32; shader.user_data_dwords().max(1) as usize];
        let out = host
            .dispatch(
                &root,
                &pso,
                &Dispatch {
                    roots: vec![(0, Root::Constants(ud))],
                    views: &[View::RawUav(1)],
                    samplers: &[],
                    groups: [1, 1, 1],
                },
            )
            .unwrap_or_else(|e| panic!("dispatch failed: {e}"));
        let got = out[0][0] & 0xFFFF;
        eprintln!("v_add_f16 0x3c03 + 0x4000 on the hardware device, nearest {nearest}: {got:#x}");
        if nearest {
            assert_eq!(got, 0x4202);
        } else {
            assert!(got == 0x4201 || got == 0x4202, "{got:#x}");
        }
    }
}

#[test]
fn a_wave_or_inside_a_lane_branch_needs_the_branch_wave_wide() {
    for (skip, name) in [(0xBF88_0000, "s_cbranch_execz"), (0xBF84_0000, "s_cbranch_scc0")] {
        a_wave_or_inside(skip, name);
    }
}

fn a_wave_or_inside(skip: u32, name: &str) {
    let native = program(skip);
    let p = ir::build(&native.code, native.wave64, native.vgprs).expect("ir::build");
    let branch = (0..p.blocks.len())
        .find(|&b| p.blocks[b].insts.last().is_some_and(|i| i.name() == name))
        .expect("the branch");
    // per lane, the two host waves cannot meet inside the branch
    lower::take_needs();
    let per_lane = lowered(&native, &p, &HashSet::new());
    assert!(
        matches!(&per_lane, Err(ShaderError::Unsupported { what, .. }) if *what == "a wave reduction where not every lane runs"),
        "{:?}",
        per_lane.err()
    );
    // ... and the lowering names the block, around which that branch is the one to widen
    let needs = lower::take_needs();
    let st = structure::structure(&p).expect("structure");
    assert_eq!(lower::branches_around(&st, &needs), Some(vec![branch]));
    // (taken, past the body, when every lane's bit is clear)
    assert_eq!(st.wave_any[branch], Some(false));
    // wave-wide, every lane runs the reduction (EXEC stays a guard on what it does there)
    let wide = HashSet::from([branch]);
    let st = structure::structure_wide(&p, &wide).expect("structure");
    let inside = match p.blocks[branch].exit {
        ir::Exit::Branch { not_taken, .. } => not_taken,
        _ => unreachable!(),
    };
    assert!(st.guards[inside].iter().any(Option::is_some));
    let shader = lowered(&native, &p, &wide).expect("lower");
    if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
        let mut copy = shader.dxil.clone();
        v.validate(&mut copy).expect("DXIL validation failed");
    }
    // On a GPU that takes waves of 32 (two host waves for the console's one): the OR is of
    // both halves, whichever half's lanes take the branch.
    let _one = super::valu_tests::ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(host) = Host::new(false) else {
        eprintln!("no SM 6.6 hardware device; not run");
        return;
    };
    let (root, pso) = match host.compute_pipeline(&shader) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("the hardware device does not take this program (waves of 32?): {e}; not run");
            return;
        }
    };
    // (threads below s0 take the branch; thread t gives bit t >> 1)
    for (below, expected) in [(64u32, u32::MAX), (40, 0x000F_FFFF), (33, 0x0001_FFFF), (10, 0x1F), (1, 1)] {
        let mut ud = vec![0u32; shader.user_data_dwords().max(1) as usize];
        for u in &shader.user_data {
            if let (RootType::Constants, 0, 0) = (u.root, u.slot, u.dword) {
                ud[u.index as usize] = below;
            }
        }
        let out = host
            .dispatch(
                &root,
                &pso,
                &Dispatch {
                    roots: vec![(0, Root::Constants(ud))],
                    views: &[View::RawUav(1)],
                    samplers: &[],
                    groups: [1, 1, 1],
                },
            )
            .unwrap_or_else(|e| panic!("dispatch failed: {e}"));
        assert_eq!(out[0][0], expected, "{name}: {below} threads in the branch");
    }
    eprintln!("ran on the hardware device ({name}): the OR of both halves in every case");
}

/// Runs `shader` on the hardware device with the root constants `s0`, `s1` and gives dword 0
/// of the UAV; None (and a line) without a device that takes it.
fn on_the_card(shader: &HostShader, constants: [u32; 2]) -> Option<u32> {
    let Ok(host) = Host::new(false) else {
        eprintln!("no SM 6.6 hardware device; not run");
        return None;
    };
    let Ok((root, pso)) = host.compute_pipeline(shader) else {
        eprintln!("the hardware device does not take this program; not run");
        return None;
    };
    let mut ud = vec![0u32; shader.user_data_dwords().max(1) as usize];
    for u in &shader.user_data {
        if let (RootType::Constants, 0, d @ 0..=1) = (u.root, u.slot, u.dword) {
            ud[u.index as usize] = constants[d as usize];
        }
    }
    let out = host
        .dispatch(
            &root,
            &pso,
            &Dispatch {
                roots: vec![(0, Root::Constants(ud))],
                views: &[View::RawUav(1)],
                samplers: &[],
                groups: [1, 1, 1],
            },
        )
        .unwrap_or_else(|e| panic!("dispatch failed: {e}"));
    Some(out[0][0])
}

/// A 32-lane console wave. A register holds a lane mask on one path (the lanes below s0)
/// and a number on the other, and is read as a number after the join, as the trace programs
/// keep their lanes' masks and their node numbers on one stack
/// (`deferred_tiled:CS_LaserLightShadowRaytracing`, s46 at 0x104, 0x75c and 0x188). The
/// mask arrives as the ballot of its lanes.
#[test]
fn a_lane_mask_joined_with_a_number_is_read_as_the_number_it_is() {
    let words = [
        0x7D88_0000, // v_cmp_gt_u32 vcc_lo, s0, v0
        0xBE94_036A, // s_mov_b32 s20, vcc_lo
        0xBF06_8001, // s_cmp_eq_u32 s1, 0
        0xBF85_0001, // s_cbranch_scc1 over the next
        0xBE94_0385, // s_mov_b32 s20, 5
        0x7E00_0214, // v_mov_b32 v0, s20
        0x38 << 26 | 0x1C << 18,
        0x80 << 24 | 2 << 16, // buffer_store_dword v0, off, s[8:11], 0
    ];
    let native = native_of(&words, false, 32);
    let p = ir::build(&native.code, native.wave64, native.vgprs).expect("ir::build");
    let shader = lowered(&native, &p, &HashSet::new()).expect("lower");
    if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
        let mut copy = shader.dxil.clone();
        v.validate(&mut copy).expect("DXIL validation failed");
    }
    let _one = super::valu_tests::ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    for (below, other, expected) in [(32u32, 0u32, u32::MAX), (10, 0, 0x3FF), (0, 0, 0), (10, 1, 5)] {
        let Some(got) = on_the_card(&shader, [below, other]) else { return };
        assert_eq!(got, expected, "{below} lanes in the mask, s1 = {other}");
    }
    eprintln!("ran on the hardware device: the mask's bits, or the number");
}

/// A 32-lane console wave. `if (t < s0) { ... SCC = (s1 == 7) }` and a branch on SCC after
/// the join: the skip brings the SCC of `s_and_saveexec` ("some lane is left") along
/// (`rtdirectionalshadows:CS_RTShadowTrace32HQ_TiledFarOnly`, 0x360-0x39c). On the console
/// the skip is taken by the wave, so with some lane inside every lane sees the inner test.
/// A lane that skipped on its own would see "some lane is left" instead: refused per lane,
/// right with the branch taken wave-wide. Lane 31 stores what it saw.
#[test]
fn scc_of_a_mask_operation_joined_with_a_scalar_test_needs_the_skip_wave_wide() {
    let words = [
        0x7D88_0000, // v_cmp_gt_u32 vcc_lo, s0, v0
        0xBE8D_3C6A, // s_and_saveexec_b32 s13, vcc_lo
        0xBF88_0001, // s_cbranch_execz over the next
        0xBF06_8701, // s_cmp_eq_u32 s1, 7
        0xBEFE_030D, // s_mov_b32 exec_lo, s13
        0xBF85_0002, // s_cbranch_scc1 to "seven"
        0x7E0A_0281, // v_mov_b32 v5, 1
        0xBF82_0001, // s_branch over the next
        0x7E0A_0282, // seven: v_mov_b32 v5, 2
        0x7DA4_009F, // v_cmpx_eq_u32 exec_lo, 31, v0
        0x38 << 26 | 0x1C << 18,
        0x80 << 24 | 2 << 16 | 5 << 8, // buffer_store_dword v5, off, s[8:11], 0
    ];
    let native = native_of(&words, false, 32);
    let p = ir::build(&native.code, native.wave64, native.vgprs).expect("ir::build");
    let branch = (0..p.blocks.len())
        .find(|&b| p.blocks[b].insts.last().is_some_and(|i| i.name() == "s_cbranch_execz"))
        .expect("the skip");
    lower::take_needs();
    let per_lane = lowered(&native, &p, &HashSet::new());
    assert!(
        matches!(&per_lane, Err(ShaderError::Unsupported { what, .. }) if *what == "a lane mask read as data"),
        "{:?}",
        per_lane.err()
    );
    let st = structure::structure(&p).expect("structure");
    assert_eq!(lower::branches_around(&st, &lower::take_needs()), Some(vec![branch]));
    let shader = lowered(&native, &p, &HashSet::from([branch])).expect("lower");
    if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
        let mut copy = shader.dxil.clone();
        v.validate(&mut copy).expect("DXIL validation failed");
    }
    let _one = super::valu_tests::ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    // (lanes below s0 inside, s1) -> what lane 31 stores: 2 if SCC was set at the branch
    for (below, s1, expected) in [(0u32, 7u32, 1u32), (10, 7, 2), (10, 3, 1), (32, 3, 1), (32, 7, 2)] {
        let Some(got) = on_the_card(&shader, [below, s1]) else { return };
        assert_eq!(got, expected, "{below} lanes inside, s1 = {s1}");
    }
    eprintln!("ran on the hardware device: the inner test's SCC for every lane once some lane is inside");
}

/// Runs a compute shader on the hardware device and returns `uav_dwords` dwords
/// from UAV 0, or `None` if no hardware device with wave32 support is present.
fn on_the_card_uav(
    shader: &HostShader,
    constants: [u32; 2],
    uav_dwords: u32,
) -> Option<Vec<u32>> {
    let Ok(host) = Host::new(false) else {
        return None;
    };
    let (root, pso) = match host.compute_pipeline(shader) {
        Ok(p) => p,
        Err(e) => panic!("hardware pipeline refused: {e}"),
    };
    let mut ud = vec![0u32; shader.user_data_dwords().max(1) as usize];
    for u in &shader.user_data {
        if let (RootType::Constants, 0, d @ 0..=1) = (u.root, u.slot, u.dword) {
            ud[u.index as usize] = constants[d as usize];
        }
    }
    let out = host
        .dispatch(
            &root,
            &pso,
            &Dispatch {
                roots: vec![(0, Root::Constants(ud))],
                views: &[View::RawUav(uav_dwords)],
                samplers: &[],
                groups: [1, 1, 1],
            },
        )
        .unwrap_or_else(|e| panic!("dispatch failed: {e}"));
    Some(out[0].clone())
}

/// Builds a wave64 compute program with `threads` threads in x:
/// - `v1 = v0 + 1000` (each thread's payload)
/// - `v2 = (v0 ^ s0) << 2` (source lane byte address)
/// - `v3 = 0x5555_5555` (sentinel kept by guarded-off destination lanes)
/// - Under `v0 < s1` (`guard`, or a `s_cbranch_execz` skip when `branch` is set):
///   `ds_bpermute_b32 v3, v2, v1 offset:offset`
/// - Every thread stores `v3` to `uav[v0]`.
fn bpermute_program(threads: u32, offset: u16, branch: bool, lds_bytes: u32) -> NativeProgram {
    let mut words = vec![
        0x4A02_00FF,
        1000, // v_add_nc_u32 v1, 1000, v0
        0x3A04_0000, // v_xor_b32 v2, s0, v0
        0x3404_0482, // v_lshlrev_b32 v2, 2, v2
        0x7E06_02FF,
        0x5555_5555, // v_mov_b32 v3, 0x5555_5555
        0x7D88_0001, // v_cmp_gt_u32 vcc, s1, v0  (v0 < s1)
        0xBE8C_246A, // s_and_saveexec_b64 s[12:13], vcc
    ];
    if branch {
        words.push(0xBF88_0002); // s_cbranch_execz over the 2-dword ds_bpermute_b32
    }
    words.extend_from_slice(&[
        0xDACC_0000 | u32::from(offset),
        (3u32 << 24) | (1u32 << 8) | 2u32, // ds_bpermute_b32 v3, v2, v1 offset:offset
        0xBEFE_040C, // s_mov_b64 exec, s[12:13]
        0x3408_0082, // v_lshlrev_b32 v4, 2, v0  (byte offset = v0 * 4)
        0x38 << 26 | 0x1C << 18 | 1 << 12,
        0x80 << 24 | 2 << 16 | 3 << 8 | 4, // buffer_store_dword v3, v4, s[8:11], 0 offen
    ]);
    let mut prog = native_of(&words, true, threads);
    if let StageIo::Cs(cs) = &mut prog.io {
        cs.rsrc2 |= (lds_bytes / 512) << 15;
    }
    prog
}

fn expected_bpermute(threads: u32, offset: u16, xor_mask: u32, active_below: u32) -> Vec<u32> {
    (0..threads)
        .map(|t| {
            if t >= active_below {
                return 0x5555_5555;
            }
            let wave_off = t & !63;
            let addr = (t ^ xor_mask).wrapping_shl(2).wrapping_add(u32::from(offset));
            let src_lane = (addr >> 2) & 63;
            let src_thread = wave_off + src_lane;
            if src_thread < threads && src_thread < active_below {
                src_thread + 1000
            } else {
                0
            }
        })
        .collect()
}

#[test]
fn bpermute_small_wave64_group_returns_zero_for_upper_lanes() {
    let prog = bpermute_program(32, 0, false, 0);
    let p = ir::build(&prog.code, prog.wave64, prog.vgprs).expect("ir::build");
    let shader = lowered(&prog, &p, &HashSet::new()).expect("lower");
    if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
        let mut copy = shader.dxil.clone();
        v.validate(&mut copy).expect("DXIL validation failed");
    }
    let _one = super::valu_tests::ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    for (xor_mask, active_below) in [(32u32, 32u32), (1, 32), (1, 16), (32, 16)] {
        let Some(got) = on_the_card_uav(&shader, [xor_mask, active_below], 32) else {
            return;
        };
        let want = expected_bpermute(32, 0, xor_mask, active_below);
        assert_eq!(got, want, "small group [32]: xor={xor_mask}, active<{active_below}");
    }
}

#[test]
fn bpermute_cross_half_on_wave32_exchanges_all_64_lanes_and_guards() {
    for (threads, offset) in [(64u32, 0u16), (64, 4), (128, 0)] {
        let prog = bpermute_program(threads, offset, false, 0);
        let p = ir::build(&prog.code, prog.wave64, prog.vgprs).expect("ir::build");
        let shader = lowered(&prog, &p, &HashSet::new()).expect("lower");
        if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
            let mut copy = shader.dxil.clone();
            v.validate(&mut copy).expect("DXIL validation failed");
        }
        let _one = super::valu_tests::ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        for (xor_mask, active_below) in [(32u32, threads), (63, threads), (32, threads / 2 + 7)] {
            let Some(got) = on_the_card_uav(&shader, [xor_mask, active_below], threads) else {
                return;
            };
            let want = expected_bpermute(threads, offset, xor_mask, active_below);
            assert_eq!(
                got, want,
                "threads={threads}, offset={offset}, xor={xor_mask}, active<{active_below}"
            );
        }
    }
}

#[test]
fn bpermute_partial_tail_wave_returns_zero_for_missing_lanes() {
    let prog = bpermute_program(96, 0, false, 0);
    let p = ir::build(&prog.code, prog.wave64, prog.vgprs).expect("ir::build");
    let shader = lowered(&prog, &p, &HashSet::new()).expect("lower");
    if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
        let mut copy = shader.dxil.clone();
        v.validate(&mut copy).expect("DXIL validation failed");
    }
    let _one = super::valu_tests::ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    for (xor_mask, active_below) in [(32u32, 96u32), (63, 96), (32, 80)] {
        let Some(got) = on_the_card_uav(&shader, [xor_mask, active_below], 96) else {
            return;
        };
        let want = expected_bpermute(96, 0, xor_mask, active_below);
        assert_eq!(got, want, "partial tail [96]: xor={xor_mask}, active<{active_below}");
    }
}

#[test]
fn bpermute_in_lane_branch_widens_and_lds_overflow_refuses() {
    // (a) `ds_bpermute_b32` inside a per-lane branch triggers `needs_every_lane`
    // and succeeds once the branch is widened with `structure_wide`.
    let prog = bpermute_program(64, 0, true, 0);
    let p = ir::build(&prog.code, prog.wave64, prog.vgprs).expect("ir::build");
    let branch = (0..p.blocks.len())
        .find(|&b| p.blocks[b].insts.last().is_some_and(|i| i.name() == "s_cbranch_execz"))
        .expect("the skip");
    lower::take_needs();
    let per_lane = lowered(&prog, &p, &HashSet::new());
    let err = match per_lane {
        Err(ShaderError::Unsupported { what, .. }) => what,
        other => panic!("expected unsupported refusal, got {:?}", other.err()),
    };
    assert_eq!(err, "a lane permute where not every lane runs");
    assert!(lower::needs_every_lane(err));
    let st = structure::structure(&p).expect("structure");
    assert_eq!(
        lower::branches_around(&st, &lower::take_needs()),
        Some(vec![branch])
    );

    let shader = lowered(&prog, &p, &HashSet::from([branch])).expect("widened lower");
    if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
        let mut copy = shader.dxil.clone();
        v.validate(&mut copy).expect("DXIL validation failed");
    }
    let _one = super::valu_tests::ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(got) = on_the_card_uav(&shader, [32, 40], 64) {
        let want = expected_bpermute(64, 0, 32, 40);
        assert_eq!(got, want, "widened lane-branch bpermute");
    }

    // (b) Pre-allocated 32 KiB LDS leaves no room for the 64-dword/wave scratch.
    let big_lds = bpermute_program(128, 0, false, 32768);
    let p_big = ir::build(&big_lds.code, big_lds.wave64, big_lds.vgprs).expect("ir::build");
    let overflow = lowered(&big_lds, &p_big, &HashSet::new());
    assert!(
        matches!(
            &overflow,
            Err(ShaderError::Unsupported { what, .. })
                if *what == "a lane permute across two host waves (not enough LDS)"
        ),
        "expected LDS overflow refusal, got {:?}",
        overflow.err()
    );
}

/// One console wave of 64 threads with 512 bytes of LDS: thread 0's lane zeroes the flag at
/// LDS 0, the threads below s0 raise it (`ds_write_b32` of 1) and read it back inside their
/// branch, and every thread reads it after the branch and stores it to the UAV. `second`: a
/// second write in the branch after the read, of that constant.
fn flag_program(second: Option<u32>) -> NativeProgram {
    let mut body = vec![
        0x7E12_0280, // v_mov_b32 v9, 0
    ];
    body.extend_from_slice(&ds(DS_WRITE_B32, 0, 9, 9)); // ds_write_b32 v9, v9
    let mut branch = vec![0x7E04_0281]; // v_mov_b32 v2, 1
    branch.extend_from_slice(&ds(DS_WRITE_B32, 0, 9, 2)); // ds_write_b32 v9, v2
    // (a scalar exit from the branch makes it blocks of their own, which only its lanes run)
    let mut rest = ds(DS_READ_B32, 3, 9, 0).to_vec(); // ds_read_b32 v3, v9
    if let Some(k) = second {
        rest.push(0x7E0C_02FF); // v_mov_b32 v6, lit
        rest.push(k);
        rest.extend_from_slice(&ds(DS_WRITE_B32, 0, 9, 6)); // ds_write_b32 v9, v6
    }
    branch.extend_from_slice(&[
        0xBF07_8001,                       // s_cmp_lg_u32 s1, 0
        0xBF85_0000 | rest.len() as u32, // s_cbranch_scc1 past the rest
    ]);
    branch.extend_from_slice(&rest);
    body.extend_from_slice(&[
        0x7D88_0000, // v_cmp_gt_u32 vcc, s0, v0
        0xBE8C_246A, // s_and_saveexec_b64 s[12:13], vcc
        0xBF88_0000 | branch.len() as u32, // s_cbranch_execz past the branch
    ]);
    body.extend_from_slice(&branch);
    body.push(0xBEFE_040C); // s_mov_b64 exec, s[12:13]
    body.extend_from_slice(&ds(DS_READ_B32, 5, 9, 0)); // ds_read_b32 v5, v9
    body.extend_from_slice(&[
        0x38 << 26 | 0x1C << 18,
        0x80 << 24 | 2 << 16 | 5 << 8, // buffer_store_dword v5, off, s[8:11], 0
    ]);
    let mut native = native(&body);
    if let StageIo::Cs(cs) = &mut native.io {
        cs.rsrc2 |= 1 << 15; // 512 bytes of LDS
    }
    native
}

/// DS `op vdst, addr, data0` at offset 0.
const fn ds(op: u8, vdst: u8, addr: u8, data0: u8) -> [u32; 2] {
    [0x36 << 26 | (op as u32) << 18, (vdst as u32) << 24 | (data0 as u32) << 8 | addr as u32]
}

const DS_WRITE_B32: u8 = 0x0D;
const DS_READ_B32: u8 = 0x36;

/// The wave size `native` lowers to on a host of 32-lane waves.
fn flag_wave(native: &NativeProgram) -> Option<u32> {
    let p = crate::spill::build(native).expect("build");
    let st = structure::structure(&p).expect("structure");
    let res = resource::track(&p, &native.layout, native.stage, Some(&st)).expect("resource::track");
    let io = io::raise(&p, native).expect("io::raise");
    let was = lower::TEST_HOST_WAVE.replace(Some(32));
    let lowered = lower::lower(&lower::Parts {
        native,
        p: &p,
        st: &st,
        io: &io,
        res: &res,
        inputs: Vec::new(),
        outputs: Vec::new(),
        prims: Vec::new(),
        mesh_stride: 0,
    });
    lower::TEST_HOST_WAVE.set(was);
    lowered.expect("lower").wave_size
}

#[test]
fn a_branch_that_only_raises_an_lds_flag_runs_in_waves_of_32() {
    // a read that misses the other host wave's write changes nothing the branch leaves behind
    assert_eq!(flag_wave(&flag_program(None)), Some(32));
    // after the read the branch writes the flag again, with the same constant
    assert_eq!(flag_wave(&flag_program(Some(1))), Some(32));
}

#[test]
fn a_branch_that_writes_lds_two_different_values_keeps_64_lanes() {
    // a second value after the read: what the flag ends up holding depends on what was read
    assert_eq!(flag_wave(&flag_program(Some(2))), Some(64));
}

/// The block of `p` closed by the branch `name`.
fn closed_by(p: &ir::Program, name: &str) -> usize {
    (0..p.blocks.len())
        .find(|&b| p.blocks[b].insts.last().is_some_and(|i| i.name() == name))
        .unwrap_or_else(|| panic!("no {name}"))
}

/// [`program`] behind `if (s1 != 0)`: the two host waves meet inside a branch each of them
/// decides from its own copy of s1. Were the copies to differ, one half would wait at a group
/// barrier for the other; so the halves take the lower half's test, exchanged like any other
/// value, and the place where they exchange it has no such branch around it in turn.
#[test]
fn a_scalar_branch_around_a_meeting_of_host_waves_is_agreed_on() {
    let mut code = vec![
        0xBF06_8001, // s_cmp_eq_u32 s1, 0
        0xBF85_0000 | 30, // s_cbranch_scc1 past the body
    ];
    code.extend(words(0xBF88_0000));
    assert_eq!(code.len(), 32);
    let native = native(&code);
    let p = ir::build(&native.code, native.wave64, native.vgprs).expect("ir::build");
    let (scalar, lane) = (closed_by(&p, "s_cbranch_scc1"), closed_by(&p, "s_cbranch_execz"));
    let wide = HashSet::from([lane]);
    let st = structure::structure_wide(&p, &wide).expect("structure");
    lower::take_meets();
    let each = lowered(&native, &p, &wide).expect("lower");
    let meets = lower::take_meets();
    // the wave-wide decision and the OR are exchanged inside the scalar branch
    assert!(!meets.is_empty() && !meets.contains(&scalar), "{meets:?}");
    assert_eq!(lower::scalar_branches_around(&st, &meets), Some(vec![scalar]));
    let agreed = HashSet::from([scalar]);
    let shader = lower::with_agreed(&agreed, || lowered(&native, &p, &wide)).expect("lower");
    let meets = lower::take_meets();
    // one more meeting, in the scalar branch's own block (here the entry block, which every
    // thread runs): no further scalar branch is around a meeting
    assert_eq!(lower::scalar_branches_around(&st, &meets), Some(vec![scalar]));
    assert_ne!(each.dxil, shader.dxil);
    if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
        let mut copy = shader.dxil.clone();
        v.validate(&mut copy).expect("DXIL validation failed");
    }
    let _one = super::valu_tests::ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    // (threads below s0 take the lane branch and OR their bits; nothing is stored with s1 = 0)
    for (below, s1, expected) in [(40u32, 1u32, 0x000F_FFFF), (40, 0, 0), (64, 5, u32::MAX), (1, 1, 1), (33, 0, 0)] {
        let Some(got) = on_the_card(&shader, [below, s1]) else { return };
        assert_eq!(got, expected, "{below} threads in the branch, s1 = {s1}");
    }
    eprintln!("ran on the hardware device: both halves follow one test of s1");
}

/// A loop the lanes leave one by one, at the branch closing the block that holds a wave OR:
/// `do { s1 += WaveActiveBitOr(bit); s0 -= 1; } while (thread < s0)`. From the second
/// iteration on only the lanes still looping reach that block, so the two host waves cannot
/// meet there (a group barrier would wait for the lanes that left) until the loop's branch is
/// taken wave-wide: every lane then stays for as long as some lane loops, as on the console.
#[test]
fn a_loop_lanes_leave_one_by_one_is_taken_wave_wide_before_host_waves_meet_in_it() {
    let mut code = vec![
        0x7E0A_0281, // v_mov_b32 v5, 1
        0x2C0C_0081, // v_lshrrev_b32 v6, 1, v0
        0x340E_0B06, // v_lshlrev_b32 v7, v6, v5
        0xBE8C_047E, // s_mov_b64 s[12:13], exec
    ];
    code.extend_from_slice(&super::wave::WAVE_OR_V7_TO_S7);
    code.extend_from_slice(&[
        0x8001_0701, // s_add_u32 s1, s1, s7
        0x8080_8100, // s_sub_u32 s0, s0, 1
        0x7DA8_0000, // v_cmpx_gt_u32 exec, s0, v0
        0xBF89_0000 | 0xFFE8, // s_cbranch_execnz to the wave OR (24 words back)
        0xBEFE_040C, // s_mov_b64 exec, s[12:13]
        0x7E00_0201, // v_mov_b32 v0, s1
        0x38 << 26 | 0x1C << 18,
        0x80 << 24 | 2 << 16, // buffer_store_dword v0, off, s[8:11], 0
    ]);
    let native = native(&code);
    let p = ir::build(&native.code, native.wave64, native.vgprs).expect("ir::build");
    let names: Vec<&str> = p.blocks.iter().flat_map(|b| &b.insts).map(|i| i.name()).collect();
    for name in ["s_add_u32", "s_sub_u32", "v_cmpx_gt_u32", "s_cbranch_execnz"] {
        assert!(names.contains(&name), "{name} in {names:?}");
    }
    let head = closed_by(&p, "s_cbranch_execnz");
    assert!(p.blocks[head].insts.iter().any(|i| i.name() == "v_readlane_b32"), "the OR is in the loop's block");
    assert!(matches!(p.blocks[head].exit, ir::Exit::Branch { taken, .. } if taken == head), "{:?}", p.blocks[head].exit);
    let st = structure::structure(&p).expect("structure");
    // (taken, round again, when some lane's bit is set)
    assert_eq!(st.wave_any[head], Some(true));
    lower::with_loop_exits(|| {
        // per lane, the block is one only some lanes run once it has come round
        lower::take_needs();
        let per_lane = lowered(&native, &p, &HashSet::new());
        assert!(
            matches!(&per_lane, Err(ShaderError::Unsupported { what, .. }) if lower::needs_every_lane(what)),
            "{:?}",
            per_lane.err()
        );
        let needs = lower::take_needs();
        assert_eq!(needs, vec![head]);
        assert_eq!(lower::branches_around(&st, &needs), Some(vec![head]));
        // wave-wide, every lane runs it every time; the halves meet there with no scalar branch
        // around them
        let wide = HashSet::from([head]);
        let st = structure::structure_wide(&p, &wide).expect("structure");
        lower::take_meets();
        let shader = lowered(&native, &p, &wide).expect("lower");
        let meets = lower::take_meets();
        assert!(meets.contains(&head), "{meets:?}");
        assert_eq!(lower::scalar_branches_around(&st, &meets), Some(Vec::new()));
        if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
            let mut copy = shader.dxil.clone();
            v.validate(&mut copy).expect("DXIL validation failed");
        }
    });
    // (the other forms keep the block as one every lane runs: see `lower::with_loop_exits`)
    assert_eq!(lower::branches_around(&st, &[head]), Some(Vec::new()));
}

/// A thread group of two console waves (128 threads) with LDS, a wave OR, and each thread's
/// own LDS word written and read back inside a per-lane branch. Run as host waves of 32, the
/// two halves of a console wave cannot be given a barrier between that write and that read
/// (not every lane is there), so the program kept its 64-lane waves, which a 32-lane host
/// refuses. Such a group's threads are in different waves on the console as well: the title
/// orders what they share with its own barriers, and the halves need none of ours.
#[test]
fn the_lds_of_a_group_of_several_console_waves_is_left_to_the_titles_barriers() {
    let mut code = vec![
        0x7E0A_0281, // v_mov_b32 v5, 1
        0x2C0C_0081, // v_lshrrev_b32 v6, 1, v0
        0x340E_0B06, // v_lshlrev_b32 v7, v6, v5
    ];
    code.extend_from_slice(&super::wave::WAVE_OR_V7_TO_S7);
    code.extend_from_slice(&[
        0x340C_0082, // v_lshlrev_b32 v6, 2, v0
        0x7D88_0000, // v_cmp_gt_u32 vcc, s0, v0
        0xBE8C_246A, // s_and_saveexec_b64 s[12:13], vcc
        0xBF88_0004, // s_cbranch_execz to the restore of EXEC
        0xD834_0000,
        0x0000_0006, // ds_write_b32 v6, v0
        0xD8D8_0000,
        0x0100_0006, // ds_read_b32 v1, v6
        0xBEFE_040C, // s_mov_b64 exec, s[12:13]
        0x7E00_0207, // v_mov_b32 v0, s7
        0x38 << 26 | 0x1C << 18,
        0x80 << 24 | 2 << 16, // buffer_store_dword v0, off, s[8:11], 0
    ]);
    let mut native = native_of(&code, true, 128);
    // (5 user SGPRs, 512 bytes of LDS)
    native.io = StageIo::Cs(ComputeIo { rsrc2: 5 << 1 | 1 << 15, threads: [128, 1, 1] });
    let p = ir::build(&native.code, native.wave64, native.vgprs).expect("ir::build");
    let names: Vec<&str> = p.blocks.iter().flat_map(|b| &b.insts).map(|i| i.name()).collect();
    for name in ["ds_write_b32", "ds_read_b32", "s_cbranch_execz"] {
        assert!(names.contains(&name), "{name} in {names:?}");
    }
    let st = structure::structure(&p).expect("structure");
    let res = resource::track(&p, &native.layout, native.stage, Some(&st)).expect("resource::track");
    let io = io::raise(&p, &native).expect("io::raise");
    let lower = || {
        let was = lower::TEST_HOST_WAVE.replace(Some(32));
        let l = lower::lower(&lower::Parts {
            native: &native,
            p: &p,
            st: &st,
            io: &io,
            res: &res,
            inputs: Vec::new(),
            outputs: Vec::new(),
            prims: Vec::new(),
            mesh_stride: 0,
        });
        lower::TEST_HOST_WAVE.set(was);
        l.expect("lower")
    };
    assert_eq!(lower().wave_size, Some(64));
    lower::take_meets();
    let halved = lower::with_group_barriers(lower).expect("not inside another");
    assert_eq!(halved.wave_size, Some(32));
    // the halves meet only to exchange the OR, in the entry block, which every thread of the
    // group runs (and `take_meets` leaves out); not at the LDS accesses
    assert_eq!(lower::take_meets(), Vec::<usize>::new());
    let mut dxil = halved.dxil;
    if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
        v.validate(&mut dxil).expect("DXIL validation failed");
    }
    // (the switch is off again)
    assert_eq!(lower().wave_size, Some(64));
}

/// `if (t < s0) { v8 = lds[s1] + <term>; s7 = v8 of the first lane; store s7 }` in a group of
/// two console waves that count their lanes (so each is two host waves that exchange), as
/// those lower it.
fn a_first_lane_read_of_a_sum(term: u32) -> Result<(), ShaderError> {
    let code = [
        0xD765_0007,
        0x0001_00C1, // v_mbcnt_lo_u32_b32 v7, -1, 0
        0x7D88_0000, // v_cmp_gt_u32 vcc, s0, v0
        0xBE8C_246A, // s_and_saveexec_b64 s[12:13], vcc
        0xBF88_0008, // s_cbranch_execz to the restore of EXEC
        0x7E10_0201, // v_mov_b32 v8, s1
        0xD8D8_0000,
        0x0800_0008, // ds_read_b32 v8, v8
        0x4A10_1000 | term, // v_add_nc_u32 v8, <term>, v8
        0x7E0E_0508, // v_readfirstlane_b32 s7, v8
        0x7E00_0207, // v_mov_b32 v0, s7
        0x38 << 26 | 0x1C << 18,
        0x80 << 24 | 2 << 16, // buffer_store_dword v0, off, s[8:11], 0
        0xBEFE_040C, // s_mov_b64 exec, s[12:13]
    ];
    let mut native = native_of(&code, true, 128);
    // (5 user SGPRs, 512 bytes of LDS)
    native.io = StageIo::Cs(ComputeIo { rsrc2: 5 << 1 | 1 << 15, threads: [128, 1, 1] });
    let p = ir::build(&native.code, native.wave64, native.vgprs).expect("ir::build");
    let halved = lower::with_group_barriers(|| lowered(&native, &p, &HashSet::new())).expect("not inside another")?;
    if let Ok(mut v) = validator::Validator::load(validator::Validator::default_path().as_deref()) {
        let mut copy = halved.dxil.clone();
        v.validate(&mut copy).expect("DXIL validation failed");
    }
    Ok(())
}

#[test]
fn a_first_lane_read_of_what_every_running_lane_holds_is_the_host_waves_own() {
    // A scalar and shared memory at a scalar address (the structure builder's sums over the
    // waves before this one): the same in every lane that took the branch, in either host wave.
    a_first_lane_read_of_a_sum(2 /* s2 */).expect("lower");
    // With the thread index, the console's first lane may be in the other host wave.
    let per_lane = a_first_lane_read_of_a_sum(0x100 /* v0 */);
    assert!(
        matches!(&per_lane, Err(ShaderError::Unsupported { what, .. }) if *what == "a first-lane read where not every lane runs"),
        "{per_lane:?}"
    );
}
