use glamx::{Vec2, Vec3};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextImmediate {
    pub position: Vec2,
    pub em_scale: f32,
    pub msdf_range_px: f32,
    pub color: Vec3,
    pub padding: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiLineImmediate {
    pub from: Vec2,
    pub to: Vec2,
    pub from_color: [f32; 4],
    pub to_color: [f32; 4],
    pub thickness: f32,
}
