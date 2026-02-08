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
}

var<immediate> pc: PushConstants;

@vertex
fn text_2d_vs_main(in: GlyphIn) -> VOut {
    var out: VOut;
    let screen_size = vec2<f32>(system.screen);
    let pos_em = vec2(in.pos_em.x, -in.pos_em.y);
    let px = pc.text_pos + pos_em * pc.em_scale;
    let vpos = vec4(px, 0.0, 1.0);
    let ndc = vec2( (vpos.x / screen_size.x) * 2.0 - 1.0,
                    1.0 - (vpos.y / screen_size.y) * 2.0);
    out.position = vec4(ndc, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

fn median3(a: vec3<f32>) -> f32 {
    return max(min(a.r, a.g), min(max(a.r, a.g), a.b));
}

@fragment
fn text_2d_fs_main(in: VOut) -> @location(0) vec4<f32> {
    let msdf = textureSampleLevel(t_diffuse, s_diffuse, in.uv, 0.0).rgb;

    let sd = median3(msdf) - 0.5;

    let tex_size = vec2<f32>(textureDimensions(t_diffuse));
    let unit_range = vec2<f32>(pc.msdf_range_px) / tex_size;
    let screen_tex_size = vec2<f32>(1.0) / fwidth(in.uv);
    let screen_px_range = max(0.5 * dot(unit_range, screen_tex_size), 1.0);

    let alpha = clamp(sd * screen_px_range + 0.5, 0.0, 1.0);

    return vec4(pc.color, alpha);
}
