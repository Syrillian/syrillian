@group(0) @binding(0) var postDepth: texture_depth_2d;
@group(0) @binding(1) var postNormal: texture_2d<f32>;
@group(0) @binding(2) var postMaterial: texture_2d<f32>; // unused, kept for layout compatibility
@group(0) @binding(3) var aoInput: texture_2d<f32>;
@group(0) @binding(4) var aoOutput: texture_storage_2d<r32float, write>;

const R: i32 = 5;
const GAUSS: array<f32, 6> = array<f32, 6>(
    0.15957691, // 0
    0.14714990, // 1
    0.11543351, // 2
    0.07702304, // 3
    0.04372828, // 4
    0.02112151  // 5
);

fn pixel_clamp(p: vec2i, size: vec2i) -> vec2i {
    return clamp(p, vec2i(0), size - vec2i(1));
}

fn normal_weight(ndot_in: f32) -> f32 {
    var x = saturate(ndot_in);
    x = x * x; // 2
    x = x * x; // 4
    x = x * x; // 8
    x = x * x; // 16
    return x;
}

fn blur_impl(center: vec2i, axis: vec2i, size_i: vec2i) -> f32 {
    let center_depth = textureLoad(postDepth, center, 0);
    if (center_depth >= 0.9999) {
        return 1.0;
    }

    let center_n = oct_decode(textureLoad(postNormal, center, 0).xy);
    let center_ao = clamp(textureLoad(aoInput, center, 0).x, 0.0, 1.0);

    let DEPTH_SIGMA_NEAR: f32 = 0.00075;
    let DEPTH_SIGMA_FAR:  f32 = 0.00800;
    let depth_sigma = mix(DEPTH_SIGMA_NEAR, DEPTH_SIGMA_FAR, center_depth * center_depth);
    let inv_depth_sigma2 = 1.0 / max(depth_sigma * depth_sigma, 1e-12);

    var sum = 0.0;
    var wsum = 0.0;

    for (var o: i32 = -R; o <= R; o = o + 1) {
        let p = pixel_clamp(center + axis * o, size_i);
        let d = textureLoad(postDepth, p, 0);
        if (d >= 0.9999) {
            continue;
        }

        let ao = clamp(textureLoad(aoInput, p, 0).x, 0.0, 1.0);

        let a = u32(abs(o));
        let w_g = GAUSS[a];

        let dz = d - center_depth;
        let w_d = exp(-(dz * dz) * inv_depth_sigma2);

        let n = oct_decode(textureLoad(postNormal, p, 0).xy);
        let w_n = normal_weight(dot(center_n, n));

        let w = w_g * w_d * w_n;

        sum += ao * w;
        wsum += w;
    }

    return sum / max(wsum, 1e-4);
}

@compute @workgroup_size(8, 8, 1)
fn cs_blur_x(@builtin(global_invocation_id) gid: vec3u) {
    let size_u = textureDimensions(aoOutput);
    if (gid.x >= size_u.x || gid.y >= size_u.y) { return; }

    let size_i = vec2i(size_u);
    let center = vec2i(gid.xy);

    let blurred = blur_impl(center, vec2i(1, 0), size_i);
    textureStore(aoOutput, center, vec4f(blurred, 0.0, 0.0, 0.0));
}

@compute @workgroup_size(8, 8, 1)
fn cs_blur_y(@builtin(global_invocation_id) gid: vec3u) {
    let size_u = textureDimensions(aoOutput);
    if (gid.x >= size_u.x || gid.y >= size_u.y) { return; }

    let size_i = vec2i(size_u);
    let center = vec2i(gid.xy);

    let blurred = blur_impl(center, vec2i(0, 1), size_i);
    textureStore(aoOutput, center, vec4f(blurred, 0.0, 0.0, 0.0));
}
