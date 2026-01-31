const PI: f32 = 3.14159265359;
const AMBIENT_STRENGTH: f32 = 0.1;
const EPS: f32 = 1e-7;

struct CubeFaceAxes {
    forward: vec3<f32>,
    up: vec3<f32>,
    face: u32,
}

fn saturate3(v: vec3<f32>) -> vec3<f32> { return clamp(v, vec3<f32>(0.0), vec3<f32>(1.0)); }
fn safe_rsqrt(x: f32) -> f32 { return inverseSqrt(max(x, 1e-8)); }
fn safe_normalize(v: vec3<f32>) -> vec3<f32> { return v * safe_rsqrt(dot(v, v)); }

fn oct_encode(n: vec3<f32>) -> vec2<f32> {
    let denom = max(abs(n.x) + abs(n.y) + abs(n.z), 1e-6);
    var v = n / denom;
    var enc = v.xy;
    if (v.z < 0.0) {
        enc = (1.0 - abs(enc.yx)) * sign(enc);
    }
    return enc;
}

// orthonormalize t against n to build a stable tbn basis
fn ortho_tangent(T: vec3<f32>, N: vec3<f32>) -> vec3<f32> {
    return safe_normalize(T - N * dot(N, T));
}

// fetch tangent-space normal and bring it to world space with a proper tbn
fn normal_from_map(
    tex: texture_2d<f32>, samp: sampler, uv: vec2<f32>,
    Nw: vec3<f32>, Tw: vec3<f32>, Bw: vec3<f32>
) -> vec3<f32> {
    let n_ts = textureSample(tex, samp, uv).xyz * 2.0 - 1.0; // [-1..1]
    let T = ortho_tangent(safe_normalize(Tw), safe_normalize(Nw));
    let B = safe_normalize(cross(Nw, T)) * sign(dot(Bw, cross(Nw, T))); // preserve handedness
    let TBN = mat3x3<f32>(T, B, safe_normalize(Nw));
    return safe_normalize(TBN * n_ts);
}

// ---------- Microfacet (GGX) BRDF ----------

fn D_ggx(NdotH: f32, a: f32) -> f32 {
    let a2 = a * a;
    let d = NdotH * NdotH * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d + EPS);
}

fn V_smith_ggx_correlated(NdotV: f32, NdotL: f32, a: f32) -> f32 {
    let a2 = a * a;
    let gv = NdotL * sqrt(a2 + (1.0 - a2) * NdotV * NdotV);
    let gl = NdotV * sqrt(a2 + (1.0 - a2) * NdotL * NdotL);
    return 0.5 / (gv + gl + EPS);
}

fn fresnel_schlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (vec3<f32>(1.0) - F0) * pow(1.0 - cosTheta, 5.0);
}

fn diffuse_lambert(base: vec3<f32>) -> vec3<f32> {
    return base / PI;
}

fn brdf_term(
    N: vec3<f32>, V: vec3<f32>, L: vec3<f32>,
    base: vec3<f32>, metallic: f32, roughness: f32
) -> vec3<f32> {
    let a = roughness * roughness;

    let NdotL = saturate(dot(N, L));
    let NdotV = saturate(dot(N, V));
    if (NdotL <= 0.0 || NdotV <= 0.0) { return vec3<f32>(0.0); }

    let H     = safe_normalize(V + L);
    let NdotH = saturate(dot(N, H));
    let LdotH = saturate(dot(L, H));

    // Specular base reflectance
    let F0 = mix(vec3<f32>(0.04), base, metallic);
    let F  = fresnel_schlick(LdotH, F0);
    let D  = D_ggx(NdotH, a);
    let Vis= V_smith_ggx_correlated(NdotV, NdotL, a);

    let spec = F * (D * Vis) * NdotL;

    // Diffuse energy only for dielectrics
    let kD = (vec3<f32>(1.0) - F) * (1.0 - metallic);
    let diff = diffuse_lambert(base) * kD * NdotL;

    return diff + spec;
}


// ---------- Tonemapping ------------

// ACES Filmic tonemapping (linear -> linear)
fn RRTAndODTFit(v: vec3<f32>) -> vec3<f32> {
    let a = v * (v + 0.0245786) - 0.000090537;
    let b = v * (0.983729 * v + 0.4329510) + 0.238081;
    return a / b;
}

// Filmic tonemap
fn tonemap_ACES(color: vec3<f32>) -> vec3<f32> {
    let ACES_IN = mat3x3<f32>(
        vec3<f32>(0.59719, 0.07600, 0.02840),
        vec3<f32>(0.35458, 0.90834, 0.13383),
        vec3<f32>(0.04823, 0.01566, 0.83777)
    );
    let ACES_OUT = mat3x3<f32>(
        vec3<f32>( 1.60475, -0.10208, -0.00327),
        vec3<f32>(-0.53108,  1.10813, -0.07276),
        vec3<f32>(-0.07367, -0.00605,  1.07602)
    );

    let v = ACES_IN * color;
    let r = RRTAndODTFit(v);
    let o = ACES_OUT * r;
    return clamp(o, vec3<f32>(0.0), vec3<f32>(1.0));
}

// Lottes "Neutral" tonemap (linear in -> linear out)
fn tonemap_neutral(x: vec3<f32>) -> vec3<f32> {
    let A = 0.22;
    let B = 0.30;
    let C = 0.10;
    let D = 0.20;
    let E = 0.01;
    let F = 0.30;
    let exposure = 1.0;
    let v = x * exposure;
    let y = ((v * (A * v + C * B) + D * E) / (v * (A * v + B) + D * F)) - (E / F);
    return clamp(y, vec3<f32>(0.0), vec3<f32>(1.0));
}

// ------------ Attenuation -------------

fn attenuation_point(distance: f32, range: f32, radius: f32) -> f32 {
    let d2 = max(distance * distance, radius * radius);
    let inv_d2 = 1.0 / d2;

    if (range <= 0.0) { return inv_d2; }

    let x = saturate(distance / max(range, 1e-6));
    let fade = (1.0 - x * x * x * x);
    let fade2 = fade * fade;
    return inv_d2 * fade2;
}

fn calculate_attenuation(distance: f32, radius: f32) -> f32 {
    if radius <= 0.0 { return 1.0; }
    let att = 1.0 / (1.0 + 0.09 * distance + 0.032 * distance * distance);
    return clamp(att, 0.0, 1.0);
}

// ----------- Shadows ---------------

fn shadow_visibility_spot(in_pos: vec3<f32>, N: vec3<f32>, L: vec3<f32>, light: Light) -> f32 {
    if (!mat_has_cast_shadows(material) || light.shadow_map_id == 0xffffffffu) { return 1.0; }

    let world_pos_bias = in_pos + N * 0.002;
    let uvz = spot_shadow_uvz(light, world_pos_bias);
    if !(all(uvz >= vec3<f32>(0.0)) && all(uvz <= vec3<f32>(1.0))) {
        return 1.0;
    }

    let slope = 1.0 - max(dot(N, L), 0.0);
    let bias  = 0.0001 * slope;
    let layer = i32(light.shadow_map_id);
    return pcf_3x3(shadow_maps, shadow_sampler, uvz.xy, uvz.z - bias, layer);
}

fn point_face_axes(dir: vec3<f32>) -> CubeFaceAxes {
    let abs_dir = abs(dir);
    var forward: vec3<f32> = vec3<f32>(0.0);
    var up: vec3<f32> = vec3<f32>(0.0);
    var face: u32 = 0u;

    if (abs_dir.x >= abs_dir.y && abs_dir.x >= abs_dir.z) {
        if (dir.x > 0.0) {
            forward = vec3<f32>(1.0, 0.0, 0.0);
            up = vec3<f32>(0.0, -1.0, 0.0);
            face = 0u;
        } else {
            forward = vec3<f32>(-1.0, 0.0, 0.0);
            up = vec3<f32>(0.0, -1.0, 0.0);
            face = 1u;
        }
    } else if (abs_dir.y >= abs_dir.x && abs_dir.y >= abs_dir.z) {
        if (dir.y > 0.0) {
            forward = vec3<f32>(0.0, 1.0, 0.0);
            up = vec3<f32>(0.0, 0.0, 1.0);
            face = 2u;
        } else {
            forward = vec3<f32>(0.0, -1.0, 0.0);
            up = vec3<f32>(0.0, 0.0, -1.0);
            face = 3u;
        }
    } else {
        if (dir.z > 0.0) {
            forward = vec3<f32>(0.0, 0.0, 1.0);
            up = vec3<f32>(0.0, -1.0, 0.0);
            face = 4u;
        } else {
            forward = vec3<f32>(0.0, 0.0, -1.0);
            up = vec3<f32>(0.0, -1.0, 0.0);
            face = 5u;
        }
    }

    return CubeFaceAxes(forward, up, face);
}

fn cube_face_axes_from_index(face: u32) -> CubeFaceAxes {
    switch (face) {
        case 0u: { return CubeFaceAxes(vec3<f32>( 1.0,  0.0,  0.0), vec3<f32>( 0.0, -1.0,  0.0), 0u); }
        case 1u: { return CubeFaceAxes(vec3<f32>(-1.0,  0.0,  0.0), vec3<f32>( 0.0, -1.0,  0.0), 1u); }
        case 2u: { return CubeFaceAxes(vec3<f32>( 0.0,  1.0,  0.0), vec3<f32>( 0.0,  0.0,  1.0), 2u); }
        case 3u: { return CubeFaceAxes(vec3<f32>( 0.0, -1.0,  0.0), vec3<f32>( 0.0,  0.0, -1.0), 3u); }
        case 4u: { return CubeFaceAxes(vec3<f32>( 0.0,  0.0,  1.0), vec3<f32>( 0.0, -1.0,  0.0), 4u); }
        default: { return CubeFaceAxes(vec3<f32>( 0.0,  0.0, -1.0), vec3<f32>( 0.0, -1.0,  0.0), 5u); }
    }
}

fn point_shadow_uvz_axes(light: Light, axes: CubeFaceAxes, world_pos: vec3<f32>) -> vec4<f32> {
    let view = view_look_at_rh(light.position, light.position + axes.forward, axes.up);
    let near = 0.05;
    let far = max(near + 0.01, light.range);
    let proj = proj_perspective(PI * 0.5, near, far);

    let clip = proj * view * vec4<f32>(world_pos, 1.0);
    let ndc  = clip.xyz / max(1e-6, clip.w);

    var uv = ndc.xy * 0.5 + 0.5;
    uv.y = 1.0 - uv.y;

    return vec4<f32>(uv, ndc.z, f32(axes.face));
}

fn point_shadow_uvz(light: Light, dir_unbiased: vec3<f32>, world_pos: vec3<f32>) -> vec4<f32> {
    let axes = point_face_axes(dir_unbiased);
    return point_shadow_uvz_axes(light, axes, world_pos);
}

fn axis_face_index(axis: u32, positive: bool) -> u32 {
    if (axis == 0u) {
        if (positive) { return 0u; }
        return 1u;
    }
    if (axis == 1u) {
        if (positive) { return 2u; }
        return 3u;
    }
    if (positive) { return 4u; }
    return 5u;
}

fn sample_point_face(
    light: Light,
    face: u32,
    world_pos_bias: vec3<f32>,
    bias: f32
) -> vec2<f32> {
    let uvz = point_shadow_uvz_axes(light, cube_face_axes_from_index(face), world_pos_bias);
    let in_bounds = all(uvz.xy >= vec2<f32>(-0.001)) && all(uvz.xy <= vec2<f32>(1.001));
    if (!in_bounds) {
        return vec2<f32>(0.0);
    }

    let layer = i32(light.shadow_map_id) + i32(face);
    let samp = pcf_3x3(shadow_maps, shadow_sampler, uvz.xy, uvz.z - bias, layer);
    return vec2<f32>(samp, 1.0);
}

fn axis_shadow_contrib(
    axis: u32,
    dir_component: f32,
    weight: f32,
    light: Light,
    world_pos_bias: vec3<f32>,
    bias: f32
) -> vec2<f32> {
    if (weight <= 1e-4) {
        return vec2<f32>(0.0);
    }

    let face = axis_face_index(axis, dir_component >= 0.0);
    let samp = sample_point_face(light, face, world_pos_bias, bias);
    return vec2<f32>(samp.x, samp.y) * weight;
}

fn shadow_visibility_point(in_pos: vec3<f32>, N: vec3<f32>, L: vec3<f32>, light: Light) -> f32 {
    if (!mat_has_cast_shadows(material) || light.shadow_map_id == 0xffffffffu) { return 1.0; }

    let dir_unbiased = in_pos - light.position;
    let dist_sq = dot(dir_unbiased, dir_unbiased);
    if (dist_sq <= 1e-8) { return 1.0; }

    let ndir = dir_unbiased * inverseSqrt(dist_sq);
    let abs_dir = abs(ndir);
    let world_pos_bias = in_pos + N * 0.002;
    let slope = 1.0 - max(dot(N, L), 0.0);
    let bias  = 0.0003 * slope;

    let contrib_x = axis_shadow_contrib(0u, ndir.x, abs_dir.x, light, world_pos_bias, bias);
    let contrib_y = axis_shadow_contrib(1u, ndir.y, abs_dir.y, light, world_pos_bias, bias);
    let contrib_z = axis_shadow_contrib(2u, ndir.z, abs_dir.z, light, world_pos_bias, bias);

    let total_weight = contrib_x.y + contrib_y.y + contrib_z.y;
    if (total_weight <= 1e-5) {
        return 1.0;
    }
    let visibility = (contrib_x.x + contrib_y.x + contrib_z.x) / total_weight;
    return visibility;
}

fn eval_spot(
    in_pos: vec3<f32>, N: vec3<f32>, V: vec3<f32>,
    base: vec3<f32>, metallic: f32, roughness: f32, light: Light
) -> vec3<f32> {
    var L = light.position - in_pos;
    let dist = length(L);
    L = L / max(dist, 1e-6);

    // Smooth spot cone
    let inner = min(light.inner_angle, light.outer_angle);
    let outer = max(light.inner_angle, light.outer_angle);
    let cosInner = cos(inner);
    let cosOuter = cos(outer);
    let dir_to_frag = safe_normalize(in_pos - light.position);
    let cosTheta = dot(safe_normalize(light.direction), dir_to_frag);
    let spot = smoothstep(cosOuter, cosInner, cosTheta);

    let radius = light.radius;
    let geom_att = attenuation_point(dist, light.range, radius);

    // Shadow
    let vis = shadow_visibility_spot(in_pos, N, L, light);

    // BRDF
    let brdf = brdf_term(N, V, L, base, metallic, roughness);

    // Radiance scaling
    let radiance = light.color * (light.intensity * geom_att) * spot * vis;

    return brdf * radiance;
}

fn eval_point(
    in_pos: vec3<f32>, N: vec3<f32>, V: vec3<f32>,
    base: vec3<f32>, metallic: f32, roughness: f32, light: Light
) -> vec3<f32> {
    var L = light.position - in_pos;
    let dist = length(L);
    L = L / max(dist, 1e-6);

    let radius = light.radius;
    let geom_att = attenuation_point(dist, light.range, radius);

    let vis = shadow_visibility_point(in_pos, N, L, light);
    let brdf = brdf_term(N, V, L, base, metallic, roughness);
    let radiance = light.color * (light.intensity * geom_att) * vis;

    return brdf * radiance;
}

fn eval_sun(
    _in_pos: vec3<f32>, N: vec3<f32>, V: vec3<f32>,
    base: vec3<f32>, metallic: f32, roughness: f32, light: Light
) -> vec3<f32> {
    let L = safe_normalize(light.direction);
    let brdf = brdf_term(N, V, L, base, metallic, roughness);
    let radiance = light.color * light.intensity;
    return brdf * radiance;
}

@fragment
fn fs_main_3d(in: FInput) -> FOutput {
    var out: FOutput;

    // Base color (linear)
    var base_rgba: vec4<f32>;
    if mat_has_texture_diffuse(material) {
        base_rgba = textureSample(t_diffuse, s_diffuse, in.uv);
    } else {
        base_rgba = vec4<f32>(material.diffuse, 1.0);
    }

    // Alpha test
    if (base_rgba.a < 0.01) { discard; }

    let base = saturate3(base_rgba.rgb);

    let metallic = clamp(material.metallic, 0.0, 1.0);

    var roughness: f32;
    if mat_has_texture_roughness(material) {
        roughness = textureSample(t_roughness, s_roughness, in.uv).g;
    } else {
        roughness = clamp(material.roughness, 0.045, 1.0);
    }

    var Lo = base;

    // World normal
    var N: vec3<f32>;
    if mat_has_texture_normal(material) {
        N = normal_from_map(t_normal, s_normal, in.uv, in.normal, in.tangent, in.bitangent);
    } else {
        N = safe_normalize(in.normal);
    }
    let V = safe_normalize(camera.position - in.position);   // to viewer
    let n_enc = oct_encode(N);
    out.out_normal = vec4(n_enc, 0.0, 1.0);
    out.out_material = vec4(roughness, metallic, 0.0, material.alpha);

    if mat_has_texture_diffuse(material) && mat_is_grayscale_diffuse(material) {
        out.out_color = vec4(vec3(base_rgba.r), base_rgba.g);
        return out;
    }

    if mat_is_lit(material) {
        // start with a dim ambient term (energy‑aware)
        Lo *= (AMBIENT_STRENGTH * (1.0 - 0.04)); // tiny spec energy loss
    }

    // Lights
    let count = light_count;
    for (var i: u32 = 0u; i < count; i = i + 1u) {
        let Ld = lights[i];
        if (Ld.type_id == LIGHT_TYPE_POINT) {
            Lo += eval_point(in.position, N, V, base, metallic, roughness, Ld);
        } else if (Ld.type_id == LIGHT_TYPE_SUN) {
            Lo += eval_sun(in.position, N, V, base, metallic, roughness, Ld);
        } else if (Ld.type_id == LIGHT_TYPE_SPOT) {
            Lo += eval_spot(in.position, N, V, base, metallic, roughness, Ld);
        }
    }

    // neutral tonemap
//    let color_tm = tonemap_neutral(Lo);

    // raw
    //let color_tm = Lo;

    // filmic tonemapping
    let color_tm = tonemap_ACES(Lo);
    out.out_color = vec4(color_tm, base_rgba.a * material.alpha);
    return out;
}


// Shadow stuff
fn view_look_at_rh(pos: vec3<f32>, target_pos: vec3<f32>, up: vec3<f32>) -> mat4x4<f32> {
    let f = normalize(target_pos - pos);
    let s = normalize(cross(f, up));
    let u = cross(s, f);

    return mat4x4<f32>(
        vec4<f32>(  s.x,   u.x,  -f.x, 0.0),
        vec4<f32>(  s.y,   u.y,  -f.y, 0.0),
        vec4<f32>(  s.z,   u.z,  -f.z, 0.0),
        vec4<f32>(-dot(s, pos),
                  -dot(u, pos),
                   dot(f, pos), 1.0)
    );
}

fn proj_perspective(fovy: f32, near: f32, far: f32) -> mat4x4<f32> {
    let sin_fov = sin(0.5 * fovy);
    let cos_fov = cos(0.5 * fovy);
    let h = cos_fov / sin_fov;
    let r = far / (near - far);
    return mat4x4<f32>(
        vec4<f32>( h, 0.0, 0.0, 0.0),
        vec4<f32>( 0.0, h, 0.0, 0.0),
        vec4<f32>( 0.0, 0.0, r, -1.0),
        vec4<f32>( 0.0, 0.0, r * near, 0.0)
    );
}

fn spot_shadow_uvz(light: Light, world_pos: vec3<f32>) -> vec3<f32> {
    let up   = light.up;
    let view = view_look_at_rh(light.position, light.position + light.direction, up);

    let fovy = max(0.0175, 2.0 * max(light.inner_angle, light.outer_angle));
    let near = 0.05;
    let far  = max(near + 0.01, light.range);
    let proj = proj_perspective(fovy, near, far);

    let clip = proj * view * vec4<f32>(world_pos, 1.0);
    let ndc  = clip.xyz / max(1e-6, clip.w);

    var uv = ndc.xy * 0.5 + 0.5;
    uv.y = 1.0 - uv.y;

    return vec3<f32>(uv, ndc.z);
}

fn pcf_3x3(depthTex: texture_depth_2d_array,
           cmpSampler: sampler_comparison,
           uv: vec2<f32>, depth_ref: f32, layer: i32) -> f32
{
    let dims  = vec2<f32>(textureDimensions(depthTex, 0));
    let texel = 1.0 / dims;
    let guard = texel * 0.5;
    let guard_max = vec2<f32>(1.0) - guard;

    var sum = 0.0;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let ofs = vec2<f32>(f32(dx), f32(dy)) * texel;
            let sample_uv = clamp(uv + ofs, guard, guard_max);
            sum += textureSampleCompare(depthTex, cmpSampler, sample_uv, layer, depth_ref);
        }
    }
    return sum / 9.0;
}
