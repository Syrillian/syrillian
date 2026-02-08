@vertex
fn fallback_vs_main(in: VInput) -> FInput {
    var out: FInput;

    let mvp_matrix = camera.view_proj_mat * model.transform;

    out.clip = mvp_matrix * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;

    return out;
}

// todo: make shadermanager be able to load vertex and fragment each and combine them in a pipeline. so i can switch 2d and 3d with the fragment shader below
@fragment
fn fallback_fs_main(in: FInput) -> @location(0) vec4<f32> {
    let tex = in.uv;
    var color = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if u32(tex.x * 10.0) % 2 == 0 && u32(tex.y * 10.0) % 2 != 0 {
        color = vec4<f32>(1.0, 0.0, 1.0, 1.0);
    } else if u32(tex.x * 10.0) % 2 != 0 && u32(tex.y * 10.0) % 2 == 0 {
        color = vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }
    return color;
}
