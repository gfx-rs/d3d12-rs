use core::mem::MaybeUninit;
use std::{convert::TryInto, mem, ptr, rc::Weak};

use d3d12::{
    AlphaMode, Blob, CmdListType, CommandAllocator, CommandList, CommandQueue, CommandQueueFlags,
    CompDevice, CpuDescriptor, DescriptorHeap, DescriptorHeapFlags, DescriptorHeapType, Device,
    Factory2, Factory4, FactoryCreationFlags, FeatureLevel, GraphicsCommandList, PipelineState,
    Priority, Resource, ResourceBarrier, RootParameter, RootSignature, RootSignatureFlags,
    RootSignatureVersion, SampleDesc, Scaling, Shader, SwapChain, SwapChain1, SwapChain3,
    SwapChainPresentFlags, SwapEffect, SwapchainDesc, WeakPtr,
};
use winapi::{
    shared::{
        dxgi::IDXGIAdapter1,
        dxgiformat::{
            DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R32G32B32A32_FLOAT,
            DXGI_FORMAT_UNKNOWN,
        },
        dxgitype::{DXGI_SAMPLE_DESC, DXGI_USAGE_SHARED},
        windef::HWND,
    },
    um::{
        d3d12::{
            ID3D12PipelineState, D3D12_CACHED_PIPELINE_STATE,
            D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF, D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED,
            D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA, D3D12_INPUT_ELEMENT_DESC,
            D3D12_INPUT_LAYOUT_DESC, D3D12_PIPELINE_STATE_FLAG_NONE,
            D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE, D3D12_RECT,
            D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES, D3D12_VIEWPORT,
        },
        dcomp::IDCompositionDevice,
    },
};
use winapi::{
    shared::{
        dxgi::IDXGIDevice,
        dxgi1_2::IDXGIFactory2,
        minwindef::{LPARAM, LRESULT, UINT, WPARAM},
    },
    um::dcomp::DCompositionCreateDevice2,
};
use winapi::{
    shared::{dxgi1_4::IDXGISwapChain3, minwindef::FALSE},
    um::{
        d3d12::{
            D3D12_BLEND_DESC, D3D12_RESOURCE_BARRIER_FLAGS, D3D12_RESOURCE_BARRIER_FLAG_NONE,
            D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_STREAM_OUTPUT_DESC,
        },
        winuser,
    },
};
use winapi::{
    shared::{dxgitype::DXGI_USAGE_RENDER_TARGET_OUTPUT, minwindef::TRUE},
    um::{
        d3d12::{
            D3D12_BLEND_OP_ADD, D3D12_BLEND_ZERO, D3D12_COLOR_WRITE_ENABLE_ALL,
            D3D12_CULL_MODE_NONE, D3D12_FILL_MODE_SOLID, D3D12_LOGIC_OP_CLEAR,
            D3D12_RASTERIZER_DESC, D3D12_RENDER_TARGET_BLEND_DESC,
        },
        dcomp::{
            DCompositionCreateDevice, DCompositionCreateDevice3, IDCompositionDevice2,
            IDCompositionDevice3,
        },
    },
};
use winapi::{um::d3d12::D3D12_GRAPHICS_PIPELINE_STATE_DESC, Interface};

const NUM_OF_FRAMES: usize = 2;

struct Window {
    factory: Option<Factory4>,
    adapter: Option<WeakPtr<IDXGIAdapter1>>,
    device: Option<Device>,
    queue: Option<CommandQueue>,
    allocator: Option<CommandAllocator>,
    list: Option<GraphicsCommandList>,
    desc_heap: Option<DescriptorHeap>,
    desc_size: Option<u32>,
    comp_device: Option<CompDevice>,
    swap_chain: Option<SwapChain3>,
    resources: Option<[Resource; NUM_OF_FRAMES]>,
    pipeline_state: Option<PipelineState>,
    root_signature: Option<RootSignature>,
}

impl Window {
    /// Create drawing resources for the window
    pub fn create_drawing_resources(&mut self, hwnd: HWND) {
        // Create Factory4
        let factory = {
            let (ptr, hr) = Factory4::create(FactoryCreationFlags::empty());
            (hr == 0).then(|| ptr)
        }
        .expect("Unable to create factory4");

        // Get any D3D adapter
        let adapter = (0..99)
            .into_iter()
            .find_map(|i| {
                let (adapter, hr) = factory.enumerate_adapters(i);
                (hr == 0).then(|| adapter)
            })
            .expect("Could not find d3d adapter");

        // Create device
        let device: Device = {
            let (ptr, hr) = Device::create(adapter, FeatureLevel::L11_0);
            (hr == 0).then(|| ptr)
        }
        .expect("Unable to create device");

        // Create command queue
        let queue = {
            let (ptr, hr) = device.create_command_queue(
                CmdListType::Direct,
                Priority::High,
                CommandQueueFlags::empty(),
                0,
            );
            (hr == 0).then(|| ptr)
        }
        .expect("Unable to create command queue");

        // Create command allocator
        let allocator = {
            let (ptr, hr) = device.create_command_allocator(CmdListType::Direct);
            (hr == 0).then(|| ptr)
        }
        .expect("Unable to create command allocator");

        // Factory 2
        let factory2: Factory2 = unsafe {
            let (ptr, hr) = factory.cast::<IDXGIFactory2>();
            (hr == 0).then(|| ptr)
        }
        .expect("Unable to cast to factory2");

        // Composition device
        let comp_device = unsafe {
            let mut ptr: WeakPtr<IDCompositionDevice> = WeakPtr::null();
            let hr = DCompositionCreateDevice(
                0 as _,
                &IDCompositionDevice::uuidof(),
                ptr.mut_void() as _,
            );
            (hr == 0).then(|| ptr)
        }
        .expect("Unable to create composition device");

        // // Create swap chain for composition
        // let swap_chain = {
        //     let (ptr, hr) = factory2.create_swapchain_for_composition_hwnd(
        //         queue,
        //         hwnd,
        //         &SwapchainDesc {
        //             width: 1024,
        //             height: 1024,
        //             format: DXGI_FORMAT_B8G8R8A8_UNORM, // Required for alpha
        //             stereo: true,
        //             sample: SampleDesc {
        //                 count: 1,
        //                 quality: 0,
        //             },
        //             buffer_usage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        //             buffer_count: NUM_OF_FRAMES as _,
        //             scaling: Scaling::Stretch,
        //             swap_effect: SwapEffect::FlipSequential, // Required for alpha
        //             alpha_mode: AlphaMode::Premultiplied,    // Required for alpha
        //             flags: 0,
        //         },
        //         comp_device,
        //     );
        //     (hr == 0).then(|| ptr)
        // }
        // .expect("Unable to create swapchain")
        // .as_swapchain0();

        // Create swap chain for composition
        let swap_chain = {
            let sw = {
                let (ptr, hr) = factory2.create_swapchain_for_hwnd(
                    queue,
                    hwnd,
                    &SwapchainDesc {
                        width: 1024,
                        height: 1024,
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        stereo: false,
                        sample: SampleDesc {
                            count: 1,
                            quality: 0,
                        },
                        buffer_usage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                        buffer_count: NUM_OF_FRAMES as _,
                        scaling: Scaling::Stretch,
                        swap_effect: SwapEffect::FlipSequential,
                        alpha_mode: AlphaMode::Ignore,
                        flags: 0,
                    },
                );
                (hr == 0).then(|| ptr)
            }
            .expect("Unable to create swapchain");
            let (ptr, hr) = unsafe { sw.cast::<IDXGISwapChain3>() };
            (hr == 0).then(|| ptr)
        }
        .expect("Unable to cast swapchain");

        // Create heap descriptor
        let desc_heap = {
            let (ptr, hr) = device.create_descriptor_heap(
                NUM_OF_FRAMES as _,
                DescriptorHeapType::Rtv,
                DescriptorHeapFlags::empty(),
                0, /* node mask */
            );
            (hr == 0).then(|| ptr)
        }
        .expect("Unable to create heap descriptor thing");

        // Create resource per frame
        let mut descriptor: CpuDescriptor = desc_heap.start_cpu_descriptor();
        let _descriptor_inc_size = device.get_descriptor_increment_size(DescriptorHeapType::Rtv);
        println!("what {}", _descriptor_inc_size);
        let resources = (0..NUM_OF_FRAMES)
            .map(|i| {
                let resource = {
                    let (ptr, hr) = swap_chain.as_swapchain0().get_buffer(i as _);
                    (hr == 0).then(|| ptr)
                }
                .expect("Unable to create resource");

                unsafe {
                    device.CreateRenderTargetView(resource.as_mut_ptr(), 0 as _, descriptor);
                    descriptor.ptr += _descriptor_inc_size as usize;
                }

                resource
            })
            .collect::<Vec<_>>()
            .try_into()
            .expect("Unable to get resources as array");

        // self.factory = Some(factory);
        self.adapter = Some(adapter);
        self.device = Some(device);
        self.queue = Some(queue);
        self.allocator = Some(allocator);
        self.desc_heap = Some(desc_heap);
        self.desc_size = Some(_descriptor_inc_size);
        self.swap_chain = Some(swap_chain);
        self.comp_device = Some(comp_device);
        self.resources = Some(resources);
    }

    pub fn load_assets(&mut self) {
        let device = self.device.unwrap();
        let allocator = self.allocator.unwrap();

        let ((root, error), hr) = RootSignature::serialize(
            RootSignatureVersion::V1_0,
            &[] as _,
            &[] as _,
            RootSignatureFlags::empty(),
        );
        if hr > 0 {
            panic!("Unable to serialize root signature");
        }

        if !error.is_null() {
            println!("err {}", unsafe { error.as_c_str().to_str().unwrap() });
            panic!("Root signature serialization error",);
        }

        let root_signature = {
            let (ptr, hr) = device.create_root_signature(root, 0);
            (hr == 0).then(|| ptr)
        }
        .expect("Unable to create root signature");

        let rtvs = [DXGI_FORMAT_UNKNOWN; 8];
        let dummy_target = D3D12_RENDER_TARGET_BLEND_DESC {
            BlendEnable: FALSE,
            LogicOpEnable: FALSE,
            SrcBlend: D3D12_BLEND_ZERO,
            DestBlend: D3D12_BLEND_ZERO,
            BlendOp: D3D12_BLEND_OP_ADD,
            SrcBlendAlpha: D3D12_BLEND_ZERO,
            DestBlendAlpha: D3D12_BLEND_ZERO,
            BlendOpAlpha: D3D12_BLEND_OP_ADD,
            LogicOp: D3D12_LOGIC_OP_CLEAR,
            RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL as _,
        };
        let render_targets = [dummy_target; 8];

        let input_elements = [D3D12_INPUT_ELEMENT_DESC {
            SemanticName: "COLOR".as_ptr() as *const _,
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
            AlignedByteOffset: 0,
            InputSlot: 0,
            InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        }];

        let pso_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            pRootSignature: root_signature.as_mut_ptr(),
            // VS: Shader::from_blob(vs),
            // PS: Shader::from_blob(ps),
            VS: *Shader::null(),
            PS: *Shader::null(),
            GS: *Shader::null(),
            DS: *Shader::null(),
            HS: *Shader::null(),
            StreamOutput: D3D12_STREAM_OUTPUT_DESC {
                pSODeclaration: ptr::null(),
                NumEntries: 0,
                pBufferStrides: ptr::null(),
                NumStrides: 0,
                RasterizedStream: 0,
            },
            BlendState: D3D12_BLEND_DESC {
                AlphaToCoverageEnable: FALSE,
                IndependentBlendEnable: FALSE,
                RenderTarget: render_targets,
            },
            SampleMask: !0,
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE,
                FrontCounterClockwise: TRUE,
                DepthBias: 0,
                DepthBiasClamp: 0.0,
                SlopeScaledDepthBias: 0.0,
                DepthClipEnable: FALSE,
                MultisampleEnable: FALSE,
                ForcedSampleCount: 0,
                AntialiasedLineEnable: FALSE,
                ConservativeRaster: D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
            },
            DepthStencilState: unsafe { mem::zeroed() },
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: input_elements.as_ptr(),
                NumElements: 1,
            },
            IBStripCutValue: D3D12_INDEX_BUFFER_STRIP_CUT_VALUE_DISABLED,
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            RTVFormats: rtvs,
            DSVFormat: DXGI_FORMAT_UNKNOWN,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            NodeMask: 0,
            CachedPSO: D3D12_CACHED_PIPELINE_STATE {
                pCachedBlob: ptr::null(),
                CachedBlobSizeInBytes: 0,
            },
            Flags: D3D12_PIPELINE_STATE_FLAG_NONE,
        };

        let mut pipeline = PipelineState::null();
        if unsafe {
            device.CreateGraphicsPipelineState(
                &pso_desc,
                &ID3D12PipelineState::uuidof(),
                pipeline.mut_void(),
            )
        } > 0
        {
            panic!("Unable to create graphics pipeline state");
        }
        // let pipeline = {
        //     let mut ptr = PipelineState::null();
        //     let hr = unsafe {
        //         device.CreateGraphicsPipelineState(
        //             &pso_desc,
        //             &ID3D12PipelineState::uuidof(),
        //             ptr.mut_void(),
        //         )
        //     };
        //     if hr > 0 {
        //         panic!("Unable to create pipeline state");
        //     }
        //     ptr
        // };

        // Create command list
        let list = {
            let (ptr, hr) =
                device.create_graphics_command_list(CmdListType::Direct, allocator, pipeline, 0);
            (hr == 0).then(|| ptr)
        }
        .expect("Unable to create command list");

        if list.close() > 0 {
            panic!("Unable to close command list");
        }

        self.root_signature = Some(root_signature);
        self.pipeline_state = Some(pipeline);
        self.list = Some(list);
    }

    pub fn populate_command_list(&mut self) {
        let allocator = self.allocator.unwrap();
        let list = self.list.unwrap();
        let resources = self.resources.unwrap();
        let swap_chain = self.swap_chain.unwrap();
        let current_frame = swap_chain.get_current_back_buffer_index() as usize;
        let current_resource = resources[current_frame];
        let desc_heap = self.desc_heap.unwrap();
        let desc_cpu = desc_heap.start_cpu_descriptor();
        let pipeline = self.pipeline_state.unwrap();
        let root_signature = self.root_signature.unwrap();

        if unsafe { allocator.Reset() } > 0 {
            panic!("allocator reset failed");
        }

        if list.reset(allocator, pipeline) > 0 {
            panic!("Unable to reset list");
        }

        list.set_graphics_root_signature(root_signature);
        // TODO:
        let viewport = D3D12_VIEWPORT {
            ..unsafe { mem::zeroed() }
        };
        unsafe {
            list.RSSetViewports(1, &viewport);
        }

        let scrects = D3D12_RECT {
            ..unsafe { mem::zeroed() }
        };
        unsafe {
            list.RSSetScissorRects(1, &scrects);
        };
        // list.set_graphics_root_shader_resource_view()
        // m_commandList->SetGraphicsRootSignature(m_rootSignature.Get());
        // m_commandList->RSSetViewports(1, &m_viewport);
        // m_commandList->RSSetScissorRects(1, &m_scissorRect);

        let barriers = [ResourceBarrier::transition(
            current_resource,
            D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            D3D12_RESOURCE_STATE_PRESENT,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_BARRIER_FLAG_NONE,
        )];
        list.resource_barrier(&barriers);

        // TODO:
        // CD3DX12_CPU_DESCRIPTOR_HANDLE rtvHandle(m_rtvHeap->GetCPUDescriptorHandleForHeapStart(), m_frameIndex, m_rtvDescriptorSize);

        // set render targets
        unsafe {
            list.OMSetRenderTargets(1, &desc_cpu, 0, ptr::null());
        }
        let bg: [f32; 4] = [1.0, 0.2, 0.4, 0.5];
        list.clear_render_target_view(desc_cpu, bg, &[]);

        // let _descriptor_inc_size = device.get_descriptor_increment_size(DescriptorHeapType::Rtv);
        // // let oo = heap.GetCPUDescriptorHandleForHeapStart();
        // list.ClearRenderTargetView(RenderTargetView, ColorRGBA, NumRects, pRects)

        let barriers = [ResourceBarrier::transition(
            current_resource,
            D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_PRESENT,
            D3D12_RESOURCE_BARRIER_FLAG_NONE,
        )];
        list.resource_barrier(&barriers);

        if list.close() > 0 {
            panic!("Unable to close command list");
        }
    }

    pub fn render(&mut self) {
        self.populate_command_list();
        let queue = self.queue.unwrap();
        let list = self.list.unwrap();
        let swap_chain = self.swap_chain.unwrap();
        let lists = [list.as_list()];
        queue.execute_command_lists(&lists);
        let hr = swap_chain
            .as_swapchain0()
            .present_flags(1, SwapChainPresentFlags::empty());
        if hr > 0 {
            panic!("Present failed");
        }
        println!("Render");
    }
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

/// Main message loop for the window
unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    #[allow(non_upper_case_globals)]
    static mut window: Window = Window {
        factory: None,
        adapter: None,
        device: None,
        queue: None,
        allocator: None,
        list: None,
        desc_heap: None,
        desc_size: None,
        comp_device: None,
        resources: None,
        swap_chain: None,
        pipeline_state: None,
        root_signature: None,
    };

    match msg {
        winuser::WM_CREATE => {
            window.create_drawing_resources(hwnd);
            window.load_assets();
            window.render();
            winuser::DefWindowProcA(hwnd, msg, wparam, lparam)
        }
        winuser::WM_PAINT => {
            window.render();
            winuser::DefWindowProcA(hwnd, msg, wparam, lparam)
            // 0
        }
        winuser::WM_DESTROY => {
            winuser::PostQuitMessage(0);
            0
        }
        _ => winuser::DefWindowProcA(hwnd, msg, wparam, lparam),
    }
}

fn main() {
    unsafe {
        let cls = winuser::WNDCLASSA {
            style: 0,
            lpfnWndProc: Some(wndproc),
            hInstance: 0 as _,
            lpszClassName: "CompositionCls\0".as_ptr() as _,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hIcon: 0 as _,
            hCursor: winuser::LoadCursorW(0 as _, winuser::IDC_ARROW as _),
            hbrBackground: 0 as _,
            lpszMenuName: 0 as _,
        };
        winuser::RegisterClassA(&cls);
        let hwnd = winuser::CreateWindowExA(
            0, //winuser::WS_EX_NOREDIRECTIONBITMAP,
            "CompositionCls\0".as_ptr() as _,
            "Composition example\0".as_ptr() as _,
            winuser::WS_OVERLAPPEDWINDOW | winuser::WS_VISIBLE,
            winuser::CW_USEDEFAULT,
            winuser::CW_USEDEFAULT,
            winuser::CW_USEDEFAULT,
            winuser::CW_USEDEFAULT,
            0 as _,
            0 as _,
            0 as _,
            0 as _,
        );
        loop {
            let mut msg = MaybeUninit::uninit();
            if winuser::GetMessageA(msg.as_mut_ptr(), hwnd, 0, 0) > 0 {
                winuser::TranslateMessage(msg.as_ptr());
                winuser::DispatchMessageA(msg.as_ptr());
            } else {
                break;
            }
        }
    }
}
