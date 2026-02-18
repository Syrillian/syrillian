@vertex
fn vs_main(in: VInput) -> FInput {
    var out: FInput;

    let p_obj = vec4(in.position, 1.0);
    let n_obj = in.normal;
    let t_obj = in.tangent.xyz;

    let ws_pos = model.transform * p_obj;
    out.position = ws_pos.xyz;
    out.clip = camera.view_proj_mat * ws_pos;

    out.uv = in.uv;

    out.normal = normalize((model.normal_mat * vec4(n_obj, 0.0)).xyz);
    out.tangent = normalize((model.normal_mat * vec4(t_obj, 0.0)).xyz);
    out.bitangent = cross(out.normal, out.tangent);

    return out;
}
