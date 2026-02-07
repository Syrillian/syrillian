struct FInput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

@group(1) @binding(0)
var postTexture: texture_2d<f32>;
@group(1) @binding(1)
var postSampler: sampler;
@group(1) @binding(2)
var postDepth: texture_depth_2d;
@group(1) @binding(3)
var postNormal: texture_2d<f32>;
@group(1) @binding(4)
var postMaterial: texture_2d<f32>;
