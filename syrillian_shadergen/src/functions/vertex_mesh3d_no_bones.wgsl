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

    // FIXME: This is only correct for uniform scaling + rotation.
    // For non-uniform scaling, transform using the inverse transpose of the model matrix (normal_mat).
    // normal_mat needs to be passed into ModelData.
    out.normal = normalize((model.transform * vec4(n_obj, 0.0)).xyz);
    out.tangent = normalize((model.transform * vec4(t_obj, 0.0)).xyz);
    out.bitangent = cross(out.normal, out.tangent);

    return out;
}
