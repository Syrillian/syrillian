use glamx::Vec3;
use num_enum::TryFromPrimitive;
use syrillian_asset::ensure_aligned;
use syrillian_macros::UniformIndex;

#[repr(C)]
#[derive(
    Debug,
    Copy,
    Clone,
    zerocopy::Immutable,
    zerocopy::IntoBytes,
    zerocopy::FromBytes,
    zerocopy::KnownLayout,
)]
pub struct LightProxy {
    pub position: Vec3,
    pub _p0: u32,
    pub up: Vec3,
    pub radius: f32,
    pub direction: Vec3,
    pub range: f32,
    pub color: Vec3,
    pub intensity: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
    pub cos_inner: f32,
    pub cos_outer: f32,
    pub type_id: u32, // LightType
    pub shadow_map_id: u32,
    pub shadow_mat_base: u32,
    pub _p1: u32,
}

impl LightProxy {
    pub const fn dummy() -> Self {
        Self {
            position: Vec3::ZERO,
            _p0: 0,
            up: Vec3::Y,
            radius: 10.0,
            direction: Vec3::NEG_Y,
            range: 10.0,
            color: Vec3::ONE,
            intensity: 1000.0,
            inner_angle: 0.0,
            outer_angle: 0.0,
            cos_inner: 1.0,
            cos_outer: 1.0,
            type_id: LightType::Point as u32,
            shadow_map_id: u32::MAX,
            shadow_mat_base: u32::MAX,
            _p1: 0,
        }
    }
}

ensure_aligned!(LightProxy { position, up, direction, color }, align <= 16 * 6 => size);

#[repr(u32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, TryFromPrimitive)]
pub enum LightType {
    Point = 0,
    Sun = 1,
    Spot = 2,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, UniformIndex)]
pub enum LightUniformIndex {
    Count = 0,
    Lights = 1,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, UniformIndex)]
pub enum ShadowUniformIndex {
    ShadowMaps = 0,
    ShadowSampler = 1,
    ShadowMatrices = 2,
    ShadowTexel = 3,
}
