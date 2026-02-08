#use model
#use default_vertex

struct FIn {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(in: VInput, @builtin(vertex_index) vid: u32) -> FIn {
    var out: FIn;

    var world_pos = vec4(in.position, 1.0);

    if vid == 0 {
        out.color = vec4(0.5, 0.0, 1.0, 1.0);
    } else {
        world_pos += vec4(normalize(in.normal) / 2, 0.0);
        out.color = vec4(0.0, 0.5, 1.0, 0.0);
    }

    out.position = camera.view_proj_mat * model.transform * world_pos;

    return out;
}

@fragment
fn fs_main(in: FIn) -> @location(0) vec4<f32> {
    return in.color;
}