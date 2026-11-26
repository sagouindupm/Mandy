//! Synthetic draw adapted from the native D3D12 view-instancing fixture.

use super::super::{
    Bytecode, PIPELINE_IIDS, StreamDesc, Translated, create_stream, library, make, pipeline,
    release, shader,
};
use super::{GraphicsState, GraphicsStreamState, HostPipeline, Sub, tests};
use crate::host::{host, transition};
use crate::object::{S_OK, This};
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use victory_shader::{HwStage, emit::HostShader};
use windows::Win32::Foundation::{CloseHandle, RECT, WAIT_OBJECT_0};
use windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIAdapter1, IDXGIFactory4,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject};
use windows::core::Interface;

const VS: &str = "float4 main(uint id : SV_VertexID) : SV_Position {
    const float2 vertices[] = {float2(-1, -1), float2(-1, 3), float2(3, -1)};
    return float4(vertices[id], 0, 1);
}";
const PS: &str = "float4 main() : SV_Target { return float4(1, 0, 0, 1); }";
const FORMAT: DXGI_FORMAT = DXGI_FORMAT_R8G8B8A8_UNORM;
const DEPTH_FORMAT: DXGI_FORMAT = DXGI_FORMAT_D32_FLOAT;

fn shader_bytes(dxc: &std::path::Path, dir: &std::path::Path) -> [Vec<u8>; 2] {
    [
        (VS, "vs_6_1", "vertex", HwStage::Vs),
        (PS, "ps_6_1", "pixel", HwStage::Ps),
    ]
    .map(|(source, profile, name, stage)| {
        let bytes = tests::compile(dxc, dir, source, profile);
        // Supply synthetic host shaders at the translation boundary.
        // PSO creation, caching, and binding still use the fork's path.
        shader(name, &bytes).translated.get_or_init(|| {
            Some(Translated {
                host: HostShader {
                    stage,
                    dxil: bytes.clone(),
                    user_data: vec![],
                    samplers: vec![],
                    static_samplers: vec![],
                    root_cbvs: vec![],
                    root_srvs: vec![],
                    root_uavs: vec![],
                    root_indexed_views: vec![],
                    half_views: vec![],
                },
                used: vec![],
                stores: false,
            })
        });
        bytes
    })
}

fn create(
    shaders: &[Vec<u8>; 2],
    routes: &[(u32, u32)],
    depth_bounds: Option<bool>,
) -> Arc<make::Pending> {
    #[repr(C, align(8))]
    struct Sub<T>(D3D12_PIPELINE_STATE_SUBOBJECT_TYPE, T);
    #[repr(C)]
    struct Stream {
        vs: Sub<Bytecode>,
        ps: Sub<Bytecode>,
        raster: Sub<D3D12_RASTERIZER_DESC>,
        depth: Sub<D3D12_DEPTH_STENCIL_DESC1>,
        topology: Sub<D3D12_PRIMITIVE_TOPOLOGY_TYPE>,
        targets: Sub<D3D12_RT_FORMAT_ARRAY>,
        dsv: Sub<DXGI_FORMAT>,
        views: Sub<D3D12_VIEW_INSTANCING_DESC>,
    }
    let [vs, ps] = shaders.each_ref().map(|bytes| Bytecode {
        code: bytes.as_ptr(),
        size: bytes.len(),
    });
    let locations: Vec<_> = routes
        .iter()
        .map(|&(viewport, slice)| D3D12_VIEW_INSTANCE_LOCATION {
            ViewportArrayIndex: viewport,
            RenderTargetArrayIndex: slice,
        })
        .collect();
    let mut depth = super::GraphicsState::default().depth1();
    depth.DepthEnable = false.into();
    if depth_bounds.is_some() && host().unwrap().depth_bounds() {
        // Keep the depth test permissive while forcing WARP to use the DSV.
        depth.DepthEnable = true.into();
        depth.DepthFunc = D3D12_COMPARISON_FUNC_ALWAYS;
    }
    depth.DepthWriteMask = D3D12_DEPTH_WRITE_MASK_ZERO;
    depth.DepthBoundsTestEnable = depth_bounds.unwrap_or(false).into();
    let mut formats = [DXGI_FORMAT_UNKNOWN; 8];
    formats[0] = FORMAT;
    let stream = Stream {
        vs: Sub(D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_VS, vs),
        ps: Sub(D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PS, ps),
        raster: Sub(
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RASTERIZER,
            D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE,
                DepthClipEnable: true.into(),
                ..Default::default()
            },
        ),
        depth: Sub(D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL1, depth),
        topology: Sub(
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_PRIMITIVE_TOPOLOGY,
            D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        ),
        targets: Sub(
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_RENDER_TARGET_FORMATS,
            D3D12_RT_FORMAT_ARRAY {
                RTFormats: formats,
                NumRenderTargets: 1,
            },
        ),
        dsv: Sub(
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_DEPTH_STENCIL_FORMAT,
            depth_bounds.map_or(DXGI_FORMAT_UNKNOWN, |_| DEPTH_FORMAT),
        ),
        views: Sub(
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_VIEW_INSTANCING,
            D3D12_VIEW_INSTANCING_DESC {
                ViewInstanceCount: locations.len() as u32,
                pViewInstanceLocations: locations.as_ptr(),
                Flags: D3D12_VIEW_INSTANCING_FLAG_NONE,
            },
        ),
    };
    let desc = StreamDesc {
        size: size_of::<Stream>()
            - if routes.is_empty() {
                size_of_val(&stream.views)
            } else {
                0
            },
        stream: (&raw const stream).cast(),
    };
    let mut raw: This = std::ptr::null();
    assert_eq!(
        create_stream(std::ptr::null(), &desc, &PIPELINE_IIDS[0], &mut raw),
        S_OK
    );
    let pending = pipeline(raw).host.clone();
    release(raw);
    pending
}

fn draw(pso: &HostPipeline, bounds: Option<[f32; 2]>) -> [[usize; 2]; 2] {
    let host = host().unwrap();
    let device = &host.device;
    const WIDTH: usize = 8;
    const HEIGHT: usize = 4;
    let desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        Width: WIDTH as u64,
        Height: HEIGHT as u32,
        DepthOrArraySize: 2,
        MipLevels: 1,
        Format: FORMAT,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
        ..Default::default()
    };
    // SAFETY: all objects and memory are owned by this draw. The fence
    // completes the readback before the buffer is mapped.
    unsafe {
        let mut target = None;
        device
            .CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_DEFAULT,
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_NONE,
                &desc,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                None,
                &mut target,
            )
            .unwrap();
        let target: ID3D12Resource = target.unwrap();
        let mut footprints = [D3D12_PLACED_SUBRESOURCE_FOOTPRINT::default(); 2];
        let mut bytes = 0;
        device.GetCopyableFootprints(
            &desc,
            0,
            2,
            0,
            Some(footprints.as_mut_ptr()),
            None,
            None,
            Some(&mut bytes),
        );
        let readback = host.read_back_buffer(bytes).unwrap();
        let heap: ID3D12DescriptorHeap = device
            .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                NumDescriptors: 1,
                ..Default::default()
            })
            .unwrap();
        let rtv = heap.GetCPUDescriptorHandleForHeapStart();
        device.CreateRenderTargetView(&target, None, rtv);

        let queue: ID3D12CommandQueue = device
            .CreateCommandQueue(&D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                ..Default::default()
            })
            .unwrap();
        let allocator: ID3D12CommandAllocator = device
            .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
            .unwrap();
        let list: ID3D12GraphicsCommandList = device
            .CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None)
            .unwrap();
        list.ClearRenderTargetView(rtv, &[0.0, 0.0, 0.0, 1.0], None);
        let depth = bounds.map(|[min, max]| {
            let mut depth = None;
            device
                .CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: D3D12_HEAP_TYPE_DEFAULT,
                        ..Default::default()
                    },
                    D3D12_HEAP_FLAG_NONE,
                    &D3D12_RESOURCE_DESC {
                        Format: DEPTH_FORMAT,
                        Flags: D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL,
                        ..desc
                    },
                    D3D12_RESOURCE_STATE_DEPTH_WRITE,
                    None,
                    &mut depth,
                )
                .unwrap();
            let depth: ID3D12Resource = depth.unwrap();
            let heap: ID3D12DescriptorHeap = device
                .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                    Type: D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
                    NumDescriptors: 1,
                    ..Default::default()
                })
                .unwrap();
            let dsv = heap.GetCPUDescriptorHandleForHeapStart();
            device.CreateDepthStencilView(&depth, None, dsv);
            list.ClearDepthStencilView(dsv, D3D12_CLEAR_FLAG_DEPTH, 0.5, 0, None);
            list.cast::<ID3D12GraphicsCommandList1>()
                .unwrap()
                .OMSetDepthBounds(min, max);
            (depth, heap, dsv)
        });
        list.OMSetRenderTargets(
            1,
            Some(&rtv),
            false,
            depth.as_ref().map(|(_, _, dsv)| dsv as *const _),
        );
        list.SetGraphicsRootSignature(&pso.root);
        list.SetPipelineState(&pso.pso);
        list.RSSetViewports(&[0.0, 4.0].map(|x| D3D12_VIEWPORT {
            TopLeftX: x,
            Width: 4.0,
            Height: 4.0,
            MaxDepth: 1.0,
            ..Default::default()
        }));
        list.RSSetScissorRects(&[0, 4].map(|x| RECT {
            left: x,
            top: 0,
            right: x + 4,
            bottom: 4,
        }));
        list.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        list.DrawInstanced(3, 1, 0, 0);
        list.ResourceBarrier(&[transition(
            &target,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_COPY_SOURCE,
        )]);
        for (slice, footprint) in footprints.iter().enumerate() {
            let source = D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(&target),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: slice as u32,
                },
            };
            let dest = D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(&readback),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: *footprint,
                },
            };
            list.CopyTextureRegion(&dest, 0, 0, 0, &source, None);
        }
        list.Close().unwrap();
        queue.ExecuteCommandLists(&[Some(list.cast().unwrap())]);
        let fence: ID3D12Fence = device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap();
        queue.Signal(&fence, 1).unwrap();
        let event = CreateEventW(None, false, false, None).unwrap();
        let waited = fence
            .SetEventOnCompletion(1, event)
            .map(|()| WaitForSingleObject(event, 10_000));
        CloseHandle(event).unwrap();
        assert_eq!(waited.unwrap(), WAIT_OBJECT_0);
        device.GetDeviceRemovedReason().unwrap();

        let mut mapped = std::ptr::null_mut();
        readback.Map(0, None, Some(&mut mapped)).unwrap();
        let image = std::slice::from_raw_parts(mapped.cast::<u8>(), bytes as usize);
        let pixels = footprints.map(|footprint| {
            let mut red = [0; 2];
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let at = footprint.Offset as usize
                        + y * footprint.Footprint.RowPitch as usize
                        + x * 4;
                    red[x / (WIDTH / 2)] += usize::from(image[at..at + 4] == [255, 0, 0, 255]);
                }
            }
            red
        });
        readback.Unmap(0, Some(&D3D12_RANGE { Begin: 0, End: 0 }));
        pixels
    }
}

#[test]
fn static_view_selection_and_library_key() {
    let locations = [
        D3D12_VIEW_INSTANCE_LOCATION {
            ViewportArrayIndex: 1,
            RenderTargetArrayIndex: 0,
        },
        D3D12_VIEW_INSTANCE_LOCATION {
            ViewportArrayIndex: 0,
            RenderTargetArrayIndex: 1,
        },
    ];
    let key = |g: &GraphicsState, stream: Option<&GraphicsStreamState<'_>>| {
        let views = stream.map_or(&[][..], |stream| stream.views);
        library::name(
            "graphics",
            b"root",
            &[b"vs", b"ps"],
            &g.describe_with_views(views),
        )
    };

    let [plain_key, depth_bounds_key] = [false, true].map(|depth_bounds| {
        let mut g = GraphicsState {
            depth_bounds,
            ..GraphicsState::default()
        };
        let masked = Sub(
            D3D12_PIPELINE_STATE_SUBOBJECT_TYPE_VIEW_INSTANCING,
            D3D12_VIEW_INSTANCING_DESC {
                ViewInstanceCount: locations.len() as u32,
                pViewInstanceLocations: locations.as_ptr(),
                Flags: D3D12_VIEW_INSTANCING_FLAG_ENABLE_VIEW_INSTANCE_MASKING,
            },
        );
        // SAFETY: the aligned subobject and its locations are live.
        unsafe { g.read_view_instancing((&raw const masked).cast()) };
        assert!(g.views.is_empty());
        let plain = g.graphics_stream_state(depth_bounds, || panic!("no views"));
        assert_eq!(plain.is_some(), depth_bounds);
        assert_eq!(
            plain
                .as_ref()
                .map(|stream| stream.depth.DepthBoundsTestEnable.as_bool()),
            depth_bounds.then_some(true),
        );
        let plain_key = key(&g, plain.as_ref());

        g.views.push(locations[0]);
        let single = g
            .graphics_stream_state(depth_bounds, || panic!("one explicit view"))
            .unwrap();
        assert_eq!(single.views, &locations[..1]);
        assert_eq!(single.depth.DepthBoundsTestEnable.as_bool(), depth_bounds);
        assert_ne!(key(&g, Some(&single)), plain_key);

        g.views.push(locations[1]);
        for tier in [Some(D3D12_VIEW_INSTANCING_TIER_NOT_SUPPORTED), None] {
            let fallback = g.graphics_stream_state(depth_bounds, || tier);
            assert_eq!(fallback.is_some(), depth_bounds);
            if let Some(stream) = &fallback {
                assert!(stream.views.is_empty());
                assert!(stream.depth.DepthBoundsTestEnable.as_bool());
            }
            assert_eq!(key(&g, fallback.as_ref()), plain_key);
        }
        for tier in [
            D3D12_VIEW_INSTANCING_TIER_1,
            D3D12_VIEW_INSTANCING_TIER_2,
            D3D12_VIEW_INSTANCING_TIER_3,
        ] {
            let stream = g
                .graphics_stream_state(depth_bounds, || Some(tier))
                .unwrap();
            assert_eq!(stream.views, &locations[..]);
            assert_eq!(stream.depth.DepthBoundsTestEnable.as_bool(), depth_bounds);
            let supported_key = key(&g, Some(&stream));
            assert_ne!(supported_key, plain_key);
            assert_eq!(
                supported_key,
                library::name("graphics", b"root", &[b"vs", b"ps"], &g.describe()),
            );
        }
        plain_key
    });
    assert_ne!(plain_key, depth_bounds_key);
}

#[test]
#[ignore = "requires --features diagnostics, VICTORY_D3D12_WARP=1 and Windows SDK DXC"]
fn static_views_through_create_stream() {
    assert!(cfg!(feature = "diagnostics") && std::env::var_os("VICTORY_D3D12_WARP").is_some());
    let device = &host().expect("WARP device").device;
    // SAFETY: querying the selected adapter and its view instancing tier.
    unsafe {
        let factory: IDXGIFactory4 = CreateDXGIFactory1().unwrap();
        let adapter: IDXGIAdapter1 = factory.EnumAdapterByLuid(device.GetAdapterLuid()).unwrap();
        let desc = adapter.GetDesc1().unwrap();
        assert_ne!(desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32, 0);
        let mut options = D3D12_FEATURE_DATA_D3D12_OPTIONS3::default();
        device
            .CheckFeatureSupport(
                D3D12_FEATURE_D3D12_OPTIONS3,
                (&raw mut options).cast(),
                size_of_val(&options) as u32,
            )
            .unwrap();
        assert_ne!(
            options.ViewInstancingTier,
            D3D12_VIEW_INSTANCING_TIER_NOT_SUPPORTED
        );
        eprintln!(
            "DXGI {:04X}:{:04X}, ViewInstancingTier={}",
            desc.VendorId, desc.DeviceId, options.ViewInstancingTier.0
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let shaders = shader_bytes(&tests::dxc().expect("Windows SDK DXC"), dir.path());
    let across = [(0, 0), (1, 0)];
    let layered = [(0, 0), (0, 1)];
    let across_pso = create(&shaders, &across, None);
    assert!(make::now(&across_pso));
    let pixels = draw(make::ready(&across_pso).unwrap(), None);
    eprintln!("two viewports, red pixels [left, right] by slice: {pixels:?}");
    assert_eq!(pixels, [[16, 16], [0, 0]]);

    let early = library::EARLY.load(Relaxed);
    let layered_pso = create(&shaders, &layered, None);
    assert_eq!(
        library::EARLY.load(Relaxed),
        early,
        "different view locations must not prewarm"
    );
    assert!(make::now(&layered_pso));
    let pixels = draw(make::ready(&layered_pso).unwrap(), None);
    eprintln!("two slices, red pixels [left, right] by slice: {pixels:?}");
    assert_eq!(pixels, [[16, 0], [16, 0]]);

    let loaded = library::LOADED.load(Relaxed);
    for (index, locations) in [across, layered].into_iter().enumerate() {
        let again = create(&shaders, &locations, None);
        assert_eq!(library::EARLY.load(Relaxed), early + index as u64 + 1);
        assert!(make::now(&again));
        assert_eq!(library::LOADED.load(Relaxed), loaded + index as u64 + 1);
    }

    let supported = host().unwrap().depth_bounds();
    eprintln!("DepthBoundsTestSupported={supported}");
    let early = library::EARLY.load(Relaxed);
    let loaded = library::LOADED.load(Relaxed);
    let range = supported.then_some([0.6, 1.0]);
    let variants = [
        (&[][..], false, [[16, 0], [0, 0]]),
        (&[][..], true, [[16, 0], [0, 0]]),
        (&across[..], false, [[16, 16], [0, 0]]),
        (&across[..], true, [[16, 16], [0, 0]]),
    ];
    for (views, bounds, unfiltered) in variants {
        let pending = create(&shaders, views, Some(bounds));
        assert_eq!(pending.graphics.as_ref().unwrap().depth_bounds, bounds);
        assert_eq!(library::EARLY.load(Relaxed), early);
        assert!(make::now(&pending));
        assert_eq!(library::LOADED.load(Relaxed), loaded);
        let pso = make::ready(&pending).unwrap();
        let pixels = draw(pso, range);
        eprintln!("views={}, depth bounds={bounds}: {pixels:?}", views.len());
        assert_eq!(
            pixels,
            if bounds && supported {
                [[0; 2]; 2]
            } else {
                unfiltered
            }
        );
        if bounds && supported && !views.is_empty() {
            let pixels = draw(pso, Some([0.4, 0.6]));
            eprintln!("views with depth inside the bounds: {pixels:?}");
            assert_eq!(pixels, unfiltered);
        }
    }
    for (index, (views, bounds, _)) in variants.into_iter().enumerate() {
        let again = create(&shaders, views, Some(bounds));
        assert_eq!(library::EARLY.load(Relaxed), early + index as u64 + 1);
        assert!(make::now(&again));
        assert_eq!(library::LOADED.load(Relaxed), loaded + index as u64 + 1);
    }
}
