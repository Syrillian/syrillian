const MAX_BONES : u32 = 256u;

struct ModelData {
    transform: mat4x4<f32>,
    normal_mat: mat4x4<f32>,
}
@group(1) @binding(0) var<uniform> model: ModelData;

struct BoneData {
    mats : array<mat4x4<f32>, MAX_BONES>,
}
@group(1) @binding(1) var<uniform> bones: BoneData;
