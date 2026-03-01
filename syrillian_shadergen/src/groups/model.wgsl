struct ModelData {
    transform: mat4x4<f32>,
    normal: mat3x3<f32>,
    pick_color: vec4<f32>,
}
@group(1) @binding(0) var<uniform> model: ModelData;
