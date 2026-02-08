#use model

struct GlyphIn {
    @location(0) pos_em: vec2<f32>,
}

struct PushConstants {
    text_pos: vec2<f32>,
    em_scale: f32,
    msdf_range_px: f32,
    color: vec3<f32>,
}

var<immediate> pc: PushConstants;



@vertex
fn vs_main(in: GlyphIn) -> @builtin(position) vec4<f32> {
    let screen_size = vec2<f32>(system.screen);
    let pos_em = vec2(in.pos_em.x, -in.pos_em.y);
    let px = pc.text_pos + pos_em * pc.em_scale;
    let vpos = model.transform * vec4(px, 0.0, 1.0);
    let ndc = vec2( (vpos.x / screen_size.x) * 2.0 - 1.0,
                    1.0 - (vpos.y / screen_size.y) * 2.0);
    return vec4(ndc, 0.0, 1.0);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4(pc.color, 1.0);
}