const AMBIENT_STRENGTH: f32 = 0.0;
const IBL_STRENGTH: f32 = 1.0;
const EPS: f32 = 1e-7;

// fetch tangent-space normal and bring it to world space with a proper tbn
fn normal_from_map(
    tex: texture_2d<f32>, samp: sampler, uv: vec2<f32>,
    N: vec3<f32>, T: vec4<f32>, B: vec3<f32>
) -> vec3<f32> {
    let n_ts = textureSample(tex, samp, uv).xyz * 2.0 - 1.0; // [-1..1]
    let TBN = mat3x3<f32>(T.xyz, B, N);
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
    let x = clamp(1.0 - cosTheta, 0.0, 1.0);
    let x2 = x * x;
    let x5 = x2 * x2 * x;
    return F0 + (vec3<f32>(1.0) - F0) * x5;
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

fn fresnel_schlick_roughness(NdotV: f32, F0: vec3<f32>, roughness: f32) -> vec3<f32> {
    let x = 1.0 - saturate(NdotV);
    let x2 = x * x;
    let x5 = x2 * x2 * x;

    let F90 = max(vec3<f32>(1.0 - roughness), F0);
    return F0 + (F90 - F0) * x5;
}

fn env_brdf_approx(roughness: f32, NdotV: f32) -> vec2<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>( 1.0,  0.0425,  1.04, -0.04);
    let r = roughness * c0 + c1;

    let a004 = min(r.x * r.x, exp2(-9.28 * saturate(NdotV))) * r.x + r.y;
    return vec2<f32>(-1.04, 1.04) * a004 + r.zw;
}

fn ibl_term(
    N: vec3<f32>,
    V: vec3<f32>,
    base: vec3<f32>,
    metallic: f32,
    roughness: f32
) -> vec3<f32> {
    let Nn = normalize(N);
    let Vn = normalize(V);

    let perceptual_roughness = clamp(roughness, 0.04, 1.0);
    let NdotV = saturate(dot(Nn, Vn));

    let F0 = mix(vec3<f32>(0.04), base, metallic);

    let Fd = fresnel_schlick_roughness(NdotV, F0, perceptual_roughness);
    let kD = (vec3<f32>(1.0) - Fd) * (1.0 - metallic);

    let mip_count = f32(textureNumLevels(skybox_map));
    let max_mip = max(mip_count - 1.0, 0.0);

    let diffuse_lod = max(max_mip - 2.0, 0.0);
    let env_diffuse = textureSampleLevel(skybox_map, skybox_sampler, Nn, diffuse_lod).rgb;

    let R = reflect(-Vn, Nn);
    let spec_lod = (perceptual_roughness * perceptual_roughness) * max_mip;
    let prefiltered = textureSampleLevel(skybox_map, skybox_sampler, R, spec_lod).rgb;

    let brdf = env_brdf_approx(perceptual_roughness, NdotV);
    let specular = prefiltered * (F0 * brdf.x + brdf.y);

    // if using real irradiance later, this becomes straight passthrough
    let diffuse = env_diffuse * base * kD;

    return (diffuse + specular) * IBL_STRENGTH;
}

fn eval_sky_sun(
    N: vec3<f32>,
    V: vec3<f32>,
    base: vec3<f32>,
    metallic: f32,
    roughness: f32
) -> vec3<f32> {
    let strength = max(sky.sun_strength, 0.0);
    if (strength <= 0.0) { return vec3<f32>(0.0); }

    let L = sky_sun_direction();
    if (L.y <= 0.0) { return vec3<f32>(0.0); }

    let NdotL = dot(N, L);
    if (NdotL <= 0.0) { return vec3<f32>(0.0); }

    let T_sun = sun_transmittance_rgb(L);

    let sun_rgb = sky_sun_color_base(L) * T_sun;

    let brdf = brdf_term(N, V, L, base, metallic, roughness);
    let radiance = sun_rgb * strength;
    return brdf * radiance;
}

// ------------ Attenuation -------------

fn attenuation_point(dist_sq: f32, range: f32, radius: f32) -> f32 {
    let r2 = radius * radius;
    let d2 = max(dist_sq, r2);
    let inv_d2 = 1.0 / d2;

    if (range <= 0.0) { return inv_d2; }

    let range2 = max(range * range, 1e-12);
    let x2 = clamp(dist_sq / range2, 0.0, 1.0);
    let x4 = x2 * x2;
    let fade = 1.0 - x4;
    return inv_d2 * fade * fade;
}

// ----------- Shadows ---------------

fn shadow_uvz_from_mat(M: mat4x4<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let clip = M * vec4<f32>(world_pos, 1.0);
    let inv_w = 1.0 / max(1e-6, clip.w);
    let ndc = clip.xyz * inv_w;

    var uv = ndc.xy * 0.5 + 0.5;
    uv.y = 1.0 - uv.y;

    return vec3<f32>(uv, ndc.z);
}

fn shadow_visibility_spot_fast(
    in_pos: vec3<f32>,
    N: vec3<f32>,
    L: vec3<f32>,
    light: Light,
    cast_shadows: bool
) -> f32 {
    if (!cast_shadows || light.shadow_map_id == 0xffffffffu || light.shadow_mat_base == 0xffffffffu) { return 1.0; }

    let slope = 1.0 - max(dot(N, L), 0.0);
    let bias  = 0.0001 * slope;

    let world_pos_bias = in_pos + N * 0.002;
    let M = shadow_mats[light.shadow_mat_base];
    let uvz = shadow_uvz_from_mat(M, world_pos_bias);

    if (!(all(uvz >= vec3<f32>(0.0)) && all(uvz <= vec3<f32>(1.0)))) {
        return 1.0;
    }

    let layer = i32(light.shadow_map_id);
    return pcf_3x3_fast(shadow_maps, shadow_sampler, uvz.xy, uvz.z - bias, layer);
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
    let mat_idx = light.shadow_mat_base + face;
    let uvz = shadow_uvz_from_mat(shadow_mats[mat_idx], world_pos_bias);
    let in_bounds = all(uvz.xy >= vec2<f32>(-0.001)) && all(uvz.xy <= vec2<f32>(1.001));
    if (!in_bounds) {
        return vec2<f32>(0.0);
    }

    let layer = i32(light.shadow_map_id) + i32(face);
    let samp = pcf_3x3_fast(shadow_maps, shadow_sampler, uvz.xy, uvz.z - bias, layer);
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

fn shadow_visibility_point(
    in_pos: vec3<f32>,
    N: vec3<f32>,
    L: vec3<f32>,
    light: Light,
    cast_shadows: bool
) -> f32 {
    if (!cast_shadows || light.shadow_map_id == 0xffffffffu || light.shadow_mat_base == 0xffffffffu) { return 1.0; }

    let ndir = -L;
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
    base: vec3<f32>, metallic: f32, roughness: f32, light: Light, cast_shadows: bool
) -> vec3<f32> {
    let toL = light.position - in_pos;
    let dist_sq = dot(toL, toL);
    if (dist_sq <= 1e-12) { return vec3<f32>(0.0); }

    let range = light.range;
    if (range > 0.0) {
        let range2 = max(range * range, 1e-12);
        if (dist_sq >= range2) { return vec3<f32>(0.0); }
    }

    let geom_att = attenuation_point(dist_sq, range, light.radius);
    if (geom_att <= 0.0) { return vec3<f32>(0.0); }

    let inv_dist = inverseSqrt(max(dist_sq, 1e-12));
    let L = toL * inv_dist;

    let cosTheta = dot(light.direction, -L);
    let spot = smoothstep(light.cos_outer, light.cos_inner, cosTheta);
    if (spot <= 0.0) { return vec3<f32>(0.0); }

    let NdotL = dot(N, L);
    if (NdotL <= 0.0) { return vec3<f32>(0.0); }

    let vis = shadow_visibility_spot_fast(in_pos, N, L, light, cast_shadows);
    let brdf = brdf_term(N, V, L, base, metallic, roughness);
    let radiance = light.color * (light.intensity * geom_att) * spot * vis;
    return brdf * radiance;
}

fn eval_point(
    in_pos: vec3<f32>, N: vec3<f32>, V: vec3<f32>,
    base: vec3<f32>, metallic: f32, roughness: f32, light: Light, cast_shadows: bool
) -> vec3<f32> {
    let toL = light.position - in_pos;
    let dist_sq = dot(toL, toL);
    if (dist_sq <= 1e-12) { return vec3<f32>(0.0); }

    let range = light.range;
    if (range > 0.0) {
        let range2 = max(range * range, 1e-12);
        if (dist_sq >= range2) { return vec3<f32>(0.0); }
    }

    let geom_att = attenuation_point(dist_sq, range, light.radius);
    if (geom_att <= 0.0) { return vec3<f32>(0.0); }

    let inv_dist = inverseSqrt(max(dist_sq, 1e-12));
    let dist = dist_sq * inv_dist;
    if (dist <= 0.0) { return vec3<f32>(0.0); }
    let L = toL * inv_dist;

    let NdotL = dot(N, L);
    if (NdotL <= 0.0) { return vec3<f32>(0.0); }

    let vis = shadow_visibility_point(in_pos, N, L, light, cast_shadows);
    let brdf = brdf_term(N, V, L, base, metallic, roughness);
    let radiance = light.color * (light.intensity * geom_att) * vis;
    return brdf * radiance;
}

fn pbr_fragment(
    in: FInput,
    base_rgba: vec4<f32>,
    normal_in: vec3<f32>,
    roughness_in: f32,
    metallic_in: f32,
    alpha_in: f32,
    lit: u32,
    cast_shadows: u32,
    grayscale_diffuse: u32
) -> FOutput {
    var out: FOutput;

    let base = saturate(base_rgba.rgb);

    let metallic = clamp(metallic_in, 0.0, 1.0);
    let roughness = clamp(roughness_in, 0.045, 1.0);

    // World normal
    let N = safe_normalize(normal_in);
    let V = safe_normalize(camera.position - in.position);   // to viewer
    let n_enc = oct_encode(N);
    out.out_normal = vec4(n_enc, 0.0, 1.0);
    out.out_material = vec4(roughness, metallic, 0.0, alpha_in);

    if grayscale_diffuse != 0 {
        out.out_color = vec4(vec3(base_rgba.r), base_rgba.g);
        return out;
    }

    var Lo = vec3<f32>(0.0);

    if lit == 0 {
        Lo = base;
    } else {
        Lo += ibl_term(N, V, base, metallic, roughness);
        Lo += base * (AMBIENT_STRENGTH * (1.0 - 0.04));
    }

    let can_cast_shadows = cast_shadows != 0;

    // Lights
    const MAX_LIGHTS: u32 = 64u;
    for (var i: u32 = 0u; i < MAX_LIGHTS; i = i + 1u) {
        if (i >= light_count) { break; }
        let Ld = lights[i];
        if (Ld.type_id == LIGHT_TYPE_POINT) {
            Lo += eval_point(in.position, N, V, base, metallic, roughness, Ld, can_cast_shadows);
        } else if (Ld.type_id == LIGHT_TYPE_SPOT) {
            Lo += eval_spot(in.position, N, V, base, metallic, roughness, Ld, can_cast_shadows);
        }
    }

    if (lit != 0) {
        Lo += eval_sky_sun(N, V, base, metallic, roughness);
    }

    out.out_color = vec4(Lo, base_rgba.a * alpha_in);
    return out;
}

fn pcf_3x3_fast(
    depthTex: texture_depth_2d_array,
    cmpSampler: sampler_comparison,
    uv: vec2<f32>,
    depth_ref: f32,
    layer: i32
) -> f32 {
    let t = shadow_texel;
    var sum = 0.0;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let ofs = vec2<f32>(f32(dx), f32(dy)) * t;
            sum += textureSampleCompare(depthTex, cmpSampler, uv + ofs, layer, depth_ref);
        }
    }
    return sum * (1.0 / 9.0);
}
