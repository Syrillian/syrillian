const LIGHT_TYPE_POINT: u32 = 0;
const LIGHT_TYPE_SUN: u32 = 1;
const LIGHT_TYPE_SPOT: u32 = 2;

struct Light {
    position: vec3<f32>,
    up: vec3<f32>,
    radius: f32,
    direction: vec3<f32>,
    range: f32,
    color: vec3<f32>,
    intensity: f32,
    inner_angle: f32,
    outer_angle: f32,
    type_id: u32,
    shadow_map_id: u32,
    view_mat: mat4x4<f32>,
}

@group(3) @binding(0) var<uniform> light_count: u32;
@group(3) @binding(1) var<storage, read> lights: array<Light>;

@group(4) @binding(0) var shadow_maps: texture_depth_2d_array;
@group(4) @binding(1) var shadow_sampler: sampler_comparison;
