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
    let world_pos = vec4(pc.text_pos + in.pos_em * pc.em_scale, 0.0, 1.0);
    return camera.view_proj_mat * model.transform * world_pos;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return vec4(pc.color, 1.0);
}
