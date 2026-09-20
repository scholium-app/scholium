//! GPU 适配器探针：枚举 wgpu 能看到的后端与适配器，并尝试创建设备。
//!
//! 用途：区分"渲染后端选择问题"和"设备节点不可见"。后者表现为适配器列表为空或只有软件适配器。
//! 在应用需要 GPU 但窗口异常的机器上先跑这个，再看候选本身。
//!
//! 运行：`cargo run --bin gpu_probe`（可在候选目录下执行）

use std::future::Future;
use std::task::{Context, Poll, Waker};

use iced::wgpu;

/// 极简阻塞执行器。探针只需要等待一次 `request_device`，不引入额外依赖。
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::hint::spin_loop(),
        }
    }
}

/// 列出适配器并逐个尝试创建设备。
pub fn main() {
    println!("wgpu 编译期启用的后端: {:?}", wgpu::Instance::enabled_backend_features());

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    println!("枚举到 {} 个适配器", adapters.len());

    let descriptor = wgpu::DeviceDescriptor {
        label: Some("gpu-probe"),
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: wgpu::MemoryHints::default(),
        trace: wgpu::Trace::Off,
    };

    for (index, adapter) in adapters.iter().enumerate() {
        let info = adapter.get_info();
        println!(
            "[{index}] backend={:?} type={:?}\n    name={}\n    driver={} ({})",
            info.backend, info.device_type, info.name, info.driver, info.driver_info
        );
        match block_on(adapter.request_device(&descriptor)) {
            Ok((_device, _queue)) => println!("    request_device: OK"),
            Err(error) => println!("    request_device: FAIL ({error})"),
        }
    }

    if adapters.is_empty() {
        println!(
            "结论：没有可用适配器。优先检查 /dev/dri 与 /dev/nvidia* 是否存在——\
             设备节点不可见时 Vulkan/GL 都无法枚举，这不是候选框架的问题。"
        );
    }
}
