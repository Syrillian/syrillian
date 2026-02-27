@vertex
fn vs_main(in: VInput) -> FInput {
    var out: FInput;

    let P_obj = vec4(in.position, 1.0);
    let N_obj = in.normal;
    let T_obj = in.tangent;

    let N = normalize(model.normal * N_obj);

    let Tm = (model.transform[0].xyz * T_obj.x)
        + (model.transform[1].xyz * T_obj.y)
        + (model.transform[2].xyz * T_obj.z);
    let T = normalize(Tm - N * dot(N, Tm));

    let B = T_obj.w * normalize(cross(N, T));

    let ws_pos = model.transform * P_obj;
    out.position = ws_pos.xyz;
    out.clip = camera.view_proj_mat * ws_pos;

    out.uv = in.uv;

    out.normal = N;
    out.tangent = vec4f(T, in.tangent.w);
    out.bitangent = B;

    return out;
}
