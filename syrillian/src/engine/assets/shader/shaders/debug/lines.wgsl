struct VSIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
}

struct VSOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(in: VSIn) -> VSOut {
    var out: VSOut;
    out.position = vec4(in.position, 1.0);
    out.color = in.color;

    out.position = camera.view_proj_mat * out.position;

    return out;
}

@fragment
fn fs_main(@location(0) color: vec4<f32>) -> @location(0) vec4<f32> {
    return color;
}