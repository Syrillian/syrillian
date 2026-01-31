@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> FInput {
    let positions = array<vec2f, 6>(
        vec2f(-1.0, -1.0),
        vec2f(1.0, -1.0),
        vec2f(-1.0, 1.0),
        vec2f(-1.0, 1.0),
        vec2f(1.0, -1.0),
        vec2f(1.0, 1.0),
    );
    let uvs = array<vec2f, 6>(
        vec2f(0.0, 1.0),
        vec2f(1.0, 1.0),
        vec2f(0.0, 0.0),
        vec2f(0.0, 0.0),
        vec2f(1.0, 1.0),
        vec2f(1.0, 0.0),
    );

    var output: FInput;
    output.position = vec4f(positions[vertex_index], 0.0, 1.0);
    output.uv = uvs[vertex_index];
    return output;
}

fn uv_to_pixel(uv: vec2f) -> vec2i {
    let size = vec2f(system.screen);
    let max_uv = (size - vec2f(1.0)) / size;
    let clamped = clamp(uv, vec2f(0.0), max_uv);
    return vec2i(clamped * size);
}

fn oct_decode(enc_in: vec2f) -> vec3f {
    let enc = clamp(enc_in, vec2f(-1.0), vec2f(1.0));
    var v = vec3f(enc.x, enc.y, 1.0 - abs(enc.x) - abs(enc.y));
    if (v.z < 0.0) {
        let v_new = (1.0 - abs(v.yx)) * sign(v.xy);
        v.x = v_new.x;
        v.y = v_new.y;
    }
    return normalize(v);
}

fn reconstruct_world(uv: vec2f, depth_ndc: f32) -> vec3f {
    let ndc = vec4f(uv * vec2f(2.0, -2.0) + vec2f(-1.0, 1.0), depth_ndc, 1.0);
    let world_h = camera.inv_view_proj_mat * ndc;
    let inv_w = 1.0 / max(abs(world_h.w), 1e-6);
    return world_h.xyz * inv_w * sign(world_h.w);
}

fn hash12(p: vec2u) -> f32 {
    var x = p.x * 1664525u + p.y * 1013904223u;
    x ^= x >> 16u;
    x *= 2246822519u;
    x ^= x >> 13u;
    return f32(x & 0x00FFFFFFu) / 16777216.0;
}

fn edge_fade(uv: vec2f, width: f32) -> f32 {
    let e = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    return saturate(e / width);
}

fn sample_reflection(uv: vec2f, roughness: f32, pixel_size: vec2f) -> vec4f {
    let r = roughness * roughness * 6.0;
    let o = pixel_size * r;

    var c = textureSample(postTexture, postSampler, uv);
    c += textureSample(postTexture, postSampler, uv + vec2f(o.x, 0.0));
    c += textureSample(postTexture, postSampler, uv - vec2f(o.x, 0.0));
    c += textureSample(postTexture, postSampler, uv + vec2f(0.0, o.y));
    c += textureSample(postTexture, postSampler, uv - vec2f(0.0, o.y));
    return c * 0.2;
}

fn depth_to_view_z(depth_ndc: f32) -> f32 {
    let n = camera.near;
    let f = camera.far;
    return (n * f) / max(f - depth_ndc * (f - n), 1e-6);
}

@fragment
fn fs_main(
    @location(0) uv: vec2f,
    @builtin(position) frag_coord: vec4f,
) -> @location(0) vec4f {
    let base_color = textureSample(postTexture, postSampler, uv);

    let pixel_i = vec2i(frag_coord.xy);
    let pixel_u = vec2u(pixel_i);

    let depth = textureLoad(postDepth, pixel_i, 0);
    if (depth >= 0.9999) {
        return base_color;
    }

    let normal_enc = textureLoad(postNormal, pixel_i, 0).xy;
    let normal = oct_decode(normal_enc);

    let material = textureLoad(postMaterial, pixel_i, 0);
    let roughness = saturate(material.x);
    let metallic  = saturate(material.y);

    let pixel_size = vec2f(1.0) / vec2f(system.screen);

    let world_pos = reconstruct_world(uv, depth);
    let view_dir  = normalize(camera.position - world_pos);
    let refl_dir  = normalize(reflect(-view_dir, normal));

    let r2 = roughness * roughness;
    let max_distance = mix(50.0, 12.0, r2);
    let steps_f = mix(64.0, 20.0, r2);
    let max_steps = i32(steps_f);
    let step_size = max_distance / steps_f;

    let origin =
        world_pos +
        normal * (0.01 + 0.05 * r2) +
        refl_dir * step_size;

    let clip_o = camera.view_proj_mat * vec4f(origin, 1.0);
    let clip_d = camera.view_proj_mat * vec4f(refl_dir, 0.0);

    let jitter = hash12(pixel_u) - 0.5;
    var t = step_size * (0.5 + 0.5 * jitter);

    var hit = false;
    var hit_uv = vec2f(0.0);
    var hit_t = 0.0;

    var has_prev = false;
    var prev_delta = 0.0;

    for (var i = 0; i < max_steps; i = i + 1) {
        let clip = clip_o + clip_d * t;
        if (clip.w <= 0.0) {
            break;
        }

        let ndc = clip.xyz / clip.w;
        if (ndc.z <= 0.0 || ndc.z >= 1.0) {
            t = t + step_size;
            has_prev = false;
            continue;
        }

        let sample_uv = vec2f(ndc.x * 0.5 + 0.5, 1.0 - (ndc.y * 0.5 + 0.5));
        if (any(sample_uv < vec2f(0.0)) || any(sample_uv > vec2f(1.0))) {
            break;
        }

        if (all(abs(sample_uv - uv) < pixel_size * 2.0)) {
            t = t + step_size;
            has_prev = false;
            continue;
        }

        let scene_depth = textureLoad(postDepth, uv_to_pixel(sample_uv), 0);
        if (scene_depth >= 0.9999) {
            t = t + step_size;
            has_prev = false;
            continue;
        }

        let delta = ndc.z - scene_depth;

        if (!has_prev) {
            prev_delta = delta;
            has_prev = true;
            t = t + step_size;
            continue;
        }

        if (delta > 0.0 && prev_delta < 0.0) {
            var t0 = max(t - step_size, 0.0);
            var t1 = t;
            var best_uv = sample_uv;

            for (var j = 0; j < 5; j = j + 1) {
                let tm = 0.5 * (t0 + t1);
                let c = clip_o + clip_d * tm;
                if (c.w <= 0.0) {
                    t0 = tm;
                    continue;
                }

                let n = c.xyz / c.w;
                let u = vec2f(n.x * 0.5 + 0.5, 1.0 - (n.y * 0.5 + 0.5));
                if (any(u < vec2f(0.0)) || any(u > vec2f(1.0))) {
                    t1 = tm;
                    continue;
                }

                let d = textureLoad(postDepth, uv_to_pixel(u), 0);
                if (d >= 0.9999) {
                    t0 = tm;
                    continue;
                }

                if (n.z > d) {
                    t1 = tm;
                    best_uv = u;
                } else {
                    t0 = tm;
                }
            }

            hit = true;
            hit_uv = best_uv;
            hit_t = t1;
            break;
        }

        prev_delta = delta;
        t = t + step_size;
    }

    if (!hit) {
        return base_color;
    }

    let final_scene_depth = textureLoad(postDepth, uv_to_pixel(hit_uv), 0);
    if (final_scene_depth >= 0.9999) {
        return base_color;
    }

    let clip_hit = clip_o + clip_d * hit_t;
    if (clip_hit.w <= 0.0) {
        return base_color;
    }

    let ndc_hit = clip_hit.xyz / clip_hit.w;

    let ray_z   = depth_to_view_z(ndc_hit.z);
    let scene_z = depth_to_view_z(final_scene_depth);

    let clip_prev = clip_o + clip_d * max(hit_t - step_size, 0.0);
    let ndc_prev  = clip_prev.xyz / max(clip_prev.w, 1e-6);
    let ray_z_prev = depth_to_view_z(ndc_prev.z);

    let step_z = abs(ray_z - ray_z_prev);

    let thickness_view =
        step_z * (1.0 + 2.0 * r2) +
        (0.02 + 0.08 * r2) * (1.0 + 0.01 * ray_z);

    if (abs(scene_z - ray_z) > thickness_view) {
        return base_color;
    }

    // ---- Shade & combine ----

    let hit_color = sample_reflection(hit_uv, roughness, pixel_size);

    let ndotv = saturate(dot(normal, view_dir));
    let fres = pow(1.0 - ndotv, 5.0);
    let f0 = mix(vec3f(0.04), base_color.rgb, metallic);
    let F = f0 + (vec3f(1.0) - f0) * fres;
    let spec = max(max(F.x, F.y), F.z);

    let fade_edge = edge_fade(hit_uv, 0.05);
    let fade_dist = 1.0 - saturate(hit_t / max_distance);
    let fade_rough = (1.0 - roughness);

    let strength = saturate(spec * fade_edge * fade_dist * fade_rough * fade_rough);

    return vec4f(mix(base_color.rgb, hit_color.rgb, strength), base_color.a);
}