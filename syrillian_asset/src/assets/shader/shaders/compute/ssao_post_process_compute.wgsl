#use render

@group(1) @binding(0) var postDepth: texture_depth_2d;
@group(1) @binding(1) var postNormal: texture_2d<f32>;
@group(1) @binding(2) var postMaterial: texture_2d<f32>;
@group(1) @binding(3) var aoInput: texture_2d<f32>;
@group(1) @binding(4) var ssaoOutput: texture_storage_2d<r32float, write>;

const TAU: f32 = 6.283185307179586;
const GOLDEN_ANGLE: f32 = 2.39996322972865332;

const SAMPLE_COUNT: u32 = 32u;

const DEPTH_GRAD_REF: f32 = 0.0025;
const SLOPE_BIAS_SCALE: f32 = 8.0;
const SLOPE_THICKNESS_SCALE: f32 = 3.0;

const SELF_NDOT_START: f32 = 0.985;
const SELF_NDOT_END:   f32 = 0.997;
const SELF_DEPTH_EPS_REL: f32 = 0.20;

const TEMPORAL_BLEND: f32 = 0.0;

fn clamp_pixel(p: vec2i, size: vec2i) -> vec2i {
    return clamp(p, vec2i(0), size - vec2i(1));
}

fn uv_to_pixel(uv: vec2f, size_f: vec2f) -> vec2i {
    let max_uv = (size_f - vec2f(1.0)) / size_f;
    let clamped = clamp(uv, vec2f(0.0), max_uv);
    return vec2i(clamped * size_f);
}

fn reconstruct_world(uv: vec2f, depth_ndc: f32) -> vec3f {
    let ndc = vec4f(uv * vec2f(2.0, -2.0) + vec2f(-1.0, 1.0), depth_ndc, 1.0);
    let world_h = camera.inv_view_proj_mat * ndc;

    let inv_w = 1.0 / max(abs(world_h.w), 1e-6);
    return world_h.xyz * inv_w * sign_not_zero_1(world_h.w);
}

fn linear_view_depth(depth_ndc: f32) -> f32 {
    let n = camera.near;
    let f = camera.far;
    let denom = f - depth_ndc * (f - n);
    return (n * f) / max(denom, 1e-6);
}

fn interleaved_gradient_noise(pixel: vec2f) -> f32 {
    return fract(52.9829189 * fract(0.06711056 * pixel.x + 0.00583715 * pixel.y));
}

@compute @workgroup_size(8, 8, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3u) {
    let out_size = textureDimensions(ssaoOutput);
    if (gid.x >= out_size.x || gid.y >= out_size.y) {
        return;
    }

    let pixel_i = vec2i(gid.xy);
    let size_i = vec2i(out_size);
    let size_f = vec2f(out_size);
    let uv = (vec2f(gid.xy) + vec2f(0.5)) / size_f;

    let depth_ndc = textureLoad(postDepth, pixel_i, 0);
    if (depth_ndc >= 0.9999) {
        textureStore(ssaoOutput, pixel_i, vec4f(1.0, 0.0, 0.0, 0.0));
        return;
    }

    let normal_ws = oct_decode(textureLoad(postNormal, pixel_i, 0).xy);
    let material = textureLoad(postMaterial, pixel_i, 0);

    let world_pos = reconstruct_world(uv, depth_ndc);
    let center_depth = linear_view_depth(depth_ndc);

    let base_radius = mix(0.12, 0.85, saturate(center_depth / 25.0));
    let view_dir = normalize(camera.position - world_pos);
    let ndot_v = saturate(abs(dot(normal_ws, view_dir)));
    let radius = base_radius * mix(0.20, 1.0, ndot_v);

    let dC = depth_ndc;
    let dR = textureLoad(postDepth, clamp_pixel(pixel_i + vec2i( 1, 0), size_i), 0);
    let dL = textureLoad(postDepth, clamp_pixel(pixel_i + vec2i(-1, 0), size_i), 0);
    let dU = textureLoad(postDepth, clamp_pixel(pixel_i + vec2i( 0, 1), size_i), 0);
    let dD = textureLoad(postDepth, clamp_pixel(pixel_i + vec2i( 0,-1), size_i), 0);

    let grad = max(max(abs(dR - dC), abs(dL - dC)), max(abs(dU - dC), abs(dD - dC)));
    let slope = saturate(grad / DEPTH_GRAD_REF);

    let base_bias = max(0.0015, radius * 0.02);
    let base_thickness = max(0.03, radius * 0.55);

    let bias_slope = 1.0 + slope * SLOPE_BIAS_SCALE;
    let thick_slope = 1.0 + slope * SLOPE_THICKNESS_SCALE;

    let intensity = 1.45;
    let power = 1.40;

    let up = select(vec3f(0.0, 0.0, 1.0), vec3f(0.0, 1.0, 0.0), abs(normal_ws.z) > 0.999);
    let t0 = normalize(cross(up, normal_ws));
    let b0 = cross(normal_ws, t0);

    let ign = interleaved_gradient_noise(vec2f(gid.xy));
    let base_angle = ign * TAU;
    let jitter = fract(ign * 7.3281) - 0.5;

    var dir2 = vec2f(cos(base_angle), sin(base_angle));
    let step_c = cos(GOLDEN_ANGLE);
    let step_s = sin(GOLDEN_ANGLE);

    var occ = 0.0;
    var wsum = 0.0;

    for (var i: u32 = 0u; i < SAMPLE_COUNT; i = i + 1u) {
        var t = (f32(i) + 0.5 + jitter) / f32(SAMPLE_COUNT);
        t = clamp(t, 0.001, 0.999);

        let disk_r = sqrt(t);
        let disk = dir2 * disk_r;
        let hemi_z = sqrt(max(0.0, 1.0 - disk_r * disk_r));

        let len_scale = mix(0.10, 1.0, t * t);

        let dir_ws = normalize(t0 * disk.x + b0 * disk.y + normal_ws * hemi_z);
        let sample_world = world_pos + dir_ws * (radius * len_scale);

        let clip = camera.view_proj_mat * vec4f(sample_world, 1.0);
        if (clip.w <= 1e-6) {
            dir2 = vec2f(dir2.x * step_c - dir2.y * step_s, dir2.x * step_s + dir2.y * step_c);
            continue;
        }

        let ndc = clip.xyz / clip.w;
        if (ndc.z < 0.0 || ndc.z > 1.0) {
            dir2 = vec2f(dir2.x * step_c - dir2.y * step_s, dir2.x * step_s + dir2.y * step_c);
            continue;
        }

        let sample_uv = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
        if (any(sample_uv < vec2f(0.0)) || any(sample_uv > vec2f(1.0))) {
            dir2 = vec2f(dir2.x * step_c - dir2.y * step_s, dir2.x * step_s + dir2.y * step_c);
            continue;
        }

        let spx = uv_to_pixel(sample_uv, size_f);
        let scene_depth_ndc = textureLoad(postDepth, spx, 0);
        if (scene_depth_ndc >= 0.9999) {
            dir2 = vec2f(dir2.x * step_c - dir2.y * step_s, dir2.x * step_s + dir2.y * step_c);
            continue;
        }

        let scene_depth = linear_view_depth(scene_depth_ndc);
        let sample_point_depth = linear_view_depth(ndc.z);

        let sample_n = oct_decode(textureLoad(postNormal, spx, 0).xy);
        let n_sim = saturate(dot(normal_ws, sample_n));
        let n_same = smoothstep(SELF_NDOT_START, SELF_NDOT_END, n_sim);

        let dz_center = abs(scene_depth - center_depth);
        let depth_close = 1.0 - saturate(dz_center / max(radius * SELF_DEPTH_EPS_REL, 1e-4));
        let same_surface = n_same * depth_close;

        let bias = base_bias * bias_slope * mix(1.0, 6.0, same_surface);
        let thickness = base_thickness * thick_slope;

        let delta = sample_point_depth - scene_depth;
        let occl = saturate((delta - bias) / thickness);

        let range_w = saturate(1.0 - dz_center / (radius * 1.75));
        let ang = saturate(dot(normal_ws, dir_ws));
        let angle_w = 0.30 + 0.70 * ang;

        let near_w = 1.0 - len_scale;
        let near_term = mix(0.35, 1.0, near_w);
        let near_w2 = near_term * near_term;

        let self_w = 1.0 - same_surface;

        let w = range_w * range_w * angle_w * near_w2 * self_w;

        occ += occl * w;
        wsum += w;

        dir2 = vec2f(dir2.x * step_c - dir2.y * step_s, dir2.x * step_s + dir2.y * step_c);
    }

    let occ_norm = occ / max(wsum, 1e-4);
    var ao = pow(saturate(1.0 - occ_norm * intensity), power);

    let roughness = saturate(material.x);
    let metallic = saturate(material.y);
    let ao_strength = mix(1.0, 0.35, metallic) * mix(0.6, 1.0, roughness);
    ao = mix(1.0, ao, ao_strength);

    if (TEMPORAL_BLEND > 0.0) {
        let prev = clamp(textureLoad(aoInput, pixel_i, 0).x, 0.0, 1.0);
        ao = mix(prev, ao, TEMPORAL_BLEND);
    }

    textureStore(ssaoOutput, pixel_i, vec4f(ao, 0.0, 0.0, 0.0));
}
