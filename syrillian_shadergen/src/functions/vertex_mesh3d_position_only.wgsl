@vertex
fn vs_main(in: VInput) -> FInput {
    var out: FInput;

    let P_obj = vec4(in.position, 1.0);

    let ws_pos = model.transform * P_obj;
    out.position = ws_pos.xyz;
    out.clip = camera.view_proj_mat * ws_pos;

    return out;
}
