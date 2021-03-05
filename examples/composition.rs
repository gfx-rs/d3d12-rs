use core::mem::MaybeUninit;
use std::{convert::TryInto, rc::Weak};

use d3d12::{
    AlphaMode, Blob, CmdListType, CommandAllocator, CommandList, CommandQueue, CommandQueueFlags,
    CompDevice, CpuDescriptor, DescriptorHeap, DescriptorHeapFlags, DescriptorHeapType, Device,
    Factory2, Factory4, FactoryCreationFlags, FeatureLevel, GraphicsCommandList, PipelineState,
    Priority, Resource, ResourceBarrier, RootParameter, RootSignatureVersion, SampleDesc, Scaling,
    SwapChain, SwapChain1, SwapChain3, SwapChainPresentFlags, SwapEffect, SwapchainDesc, WeakPtr,
};
use winapi::Interface;
use winapi::{
    shared::dxgi1_4::IDXGISwapChain3,
    um::{
        d3d12::{
            D3D12_RESOURCE_BARRIER_FLAGS, D3D12_RESOURCE_BARRIER_FLAG_NONE,
            D3D12_RESOURCE_STATE_PRESENT, D3D12_RESOURCE_STATE_RENDER_TARGET,
        },
        winuser,
    },
};
use winapi::{
    shared::dxgitype::DXGI_USAGE_RENDER_TARGET_OUTPUT,
    um::dcomp::{
        DCompositionCreateDevice, DCompositionCreateDevice3, IDCompositionDevice2,
        IDCompositionDevice3,
    },
};
use winapi::{
    shared::{
        dxgi::IDXGIAdapter1,
        dxgiformat::{DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM},
        dxgitype::DXGI_USAGE_SHARED,
        windef::HWND,
    },
    um::dcomp::IDCompositionDevice,
};
use winapi::{
    shared::{
        dxgi::IDXGIDevice,
        dxgi1_2::IDXGIFactory2,
        minwindef::{LPARAM, LRESULT, UINT, WPARAM},
    },
    um::dcomp::DCompositionCreateDevice2,
};

const NUM_OF_FRAMES: usize = 2;

struct Window {
    factory: Option<Factory4>,
    adapter: Option<WeakPtr<IDXGIAdapter1>>,
    device: Option<Device>,
    queue: Option<CommandQueue>,
    allocator: Option<CommandAllocator>,
    list: Option<GraphicsCommandList>,
    desc_heap: Option<DescriptorHeap>,
    comp_device: Option<CompDevice>,
    swap_chain: Option<SwapChain3>,
    resources: Option<[Resource; NUM_OF_FRAMES]>,
    current_frame: usize,
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

        // Create command list
        let list = {
            let (ptr, hr) = device.create_graphics_command_list(
                CmdListType::Direct,
                allocator,
                PipelineState::null(),
                0,
            );
            (hr == 0).then(|| ptr)
        }
        .expect("Unable to create command list");

        list.close();

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
                        stereo: true,
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
        let descriptor: CpuDescriptor = desc_heap.start_cpu_descriptor();
        let _descriptor_inc_size = device.get_descriptor_increment_size(DescriptorHeapType::Rtv);
        let resources = (0..NUM_OF_FRAMES)
            .map(|i| {
                let resource = {
                    let (ptr, hr) = swap_chain.as_swapchain0().get_buffer(i as _);
                    (hr == 0).then(|| ptr)
                }
                .expect("Unable to create resource");

                unsafe {
                    device.CreateRenderTargetView(resource.as_mut_ptr(), 0 as _, descriptor);

                    // TODO: Cast descriptor as CD3DX12_CPU_DESCRIPTOR_HANDLE and call
                    // descriptor.Offset(1, _descriptor_inc_size);
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
        self.list = Some(list);
        self.desc_heap = Some(desc_heap);
        self.swap_chain = Some(swap_chain);
        self.comp_device = Some(comp_device);
        self.resources = Some(resources);
    }

    pub fn load_assets(&self) {
        // _
        // TODO: Load something...
        let root_signature_raw = Blob::null();
        let error = d3d12::Error::null();
        let device = self.device.unwrap();

        // RootSignature::serialize(
        //     RootSignatureVersion::V1_0,
        //     RootParameter::constants(visibility, binding, num)

        // )

        // let root_signature = {
        //     let (ptr, hr) = device.create_root_signature(root_signature_raw, 0);
        //     (hr == 0).then(|| ptr)
        // }
        // .expect("Unable to create root signature");
    }

    pub fn populate_command_list(&mut self) {
        let allocator = self.allocator.unwrap();
        let list = self.list.unwrap();
        let resources = self.resources.unwrap();
        let current_frame = self.current_frame;
        let current_resource = resources[current_frame];
        let heap = self.desc_heap.unwrap();

        allocator.reset();

        let barriers = [ResourceBarrier::transition(
            current_resource,
            0,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_PRESENT,
            D3D12_RESOURCE_BARRIER_FLAG_NONE,
        )];
        list.resource_barrier(&barriers);

        // current_resource.GetDesc();

        unsafe {
            let desc = heap.start_cpu_descriptor();
            let bg = [0.0, 0.2, 0.4, 1.0];
            list.ClearRenderTargetView(desc, &bg, 0, 0 as _);
        }

        // let _descriptor_inc_size = device.get_descriptor_increment_size(DescriptorHeapType::Rtv);
        // // let oo = heap.GetCPUDescriptorHandleForHeapStart();
        // list.ClearRenderTargetView(RenderTargetView, ColorRGBA, NumRects, pRects)

        let barriers = [ResourceBarrier::transition(
            current_resource,
            0,
            D3D12_RESOURCE_STATE_PRESENT,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_BARRIER_FLAG_NONE,
        )];
        list.resource_barrier(&barriers);

        list.close();
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

        self.current_frame = swap_chain.get_current_back_buffer_index() as usize;
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
        comp_device: None,
        resources: None,
        swap_chain: None,
        current_frame: 0,
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
