#use model
#use material_textures

struct GlyphIn {
    @location(0) pos_em: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct PushConstants {
    text_pos: vec2<f32>,
    em_scale: f32,
    msdf_range_px: f32,
    color: vec3<f32>,
    _padding: u32,
}

var<immediate> pc: PushConstants;

@vertex
fn vs_main(in: GlyphIn) -> VOut {
    var out: VOut;
    let world_pos = vec4(pc.text_pos + in.pos_em * pc.em_scale, 0.0, 1.0);
    out.position = camera.view_proj_mat * model.transform * world_pos;
    out.uv = in.uv;
    return out;
}

fn median3(a: vec3<f32>) -> f32 {
    return max(min(a.r, a.g), min(max(a.r, a.g), a.b));
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let msdf = textureSample(t_diffuse, s_diffuse, in.uv).rgb;
    let sig = median3(msdf);
    let dist = (sig - 0.5) * pc.msdf_range_px;
    let w = max(fwidth(dist), 1e-4);
    let alpha = smoothstep(-w, w, dist);

    if (alpha <= 0.01) {
        discard;
    }
    return vec4(pc.color, 1.0);
}

