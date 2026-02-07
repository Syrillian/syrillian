const MAX_BONES : u32 = 256u;

struct ModelData {
    transform: mat4x4<f32>,
    // For correct normal transformation with non-uniform scaling,
    // add the inverse transpose of the upper 3x3 model matrix:
    // normal_mat: mat3x3<f32>,
}
@group(1) @binding(0) var<uniform> model: ModelData;

struct BoneData {
    mats : array<mat4x4<f32>, MAX_BONES>,
}
@group(1) @binding(1) var<uniform> bones: BoneData;
