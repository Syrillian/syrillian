// todo: make shadermanager be able to load vertex and fragment each and combine them in a pipeline. so i can switch 2d and 3d with the fragment shader below
@fragment
fn fallback_fs_main(in: FInput) -> FOutput {
    var out: FOutput;
    let tex = in.uv;

    var color = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if u32(tex.x * 10.0) % 2 == 0 && u32(tex.y * 10.0) % 2 != 0 {
        color = vec4<f32>(1.0, 0.0, 1.0, 1.0);
    } else if u32(tex.x * 10.0) % 2 != 0 && u32(tex.y * 10.0) % 2 == 0 {
        color = vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }

    out.out_color = color;
    out.out_normal = vec4(oct_encode(in.normal), 0.0, 1.0);
    out.out_material = vec4(1.0, 1.0, 0.0, 1.0);

    return out;
}
