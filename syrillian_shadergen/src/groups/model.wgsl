struct ModelData {
    transform: mat4x4<f32>,
    normal_mat: mat4x4<f32>,
}
@group(1) @binding(0) var<uniform> model: ModelData;
