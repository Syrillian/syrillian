use wgpu::Backends;

pub fn first_backend_to_str(backends: Backends) -> &'static str {
    match backends.iter().next() {
        Some(Backends::METAL) => "metal",
        Some(Backends::DX12) => "dx12",
        Some(Backends::GL) => "opengl",
        Some(Backends::VULKAN) => "vulkan",
        _ => "",
    }
}
