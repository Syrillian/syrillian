fn normalize_weights(w_in: vec4<f32>) -> vec4<f32> {
    let w = max(w_in, vec4<f32>(0.0));
    let s = sum4(w);
    if (s < 1e-8) {
        return vec4<f32>(0.0);
    }
    return w / s;
}

fn skin_pos(p: vec4<f32>, idx: vec4<u32>, ow: vec4<f32>) -> vec4<f32> {
    let w = normalize_weights(ow);
    if (sum4(w) == 0.0) {
        return p;
    }

    var r = vec4<f32>(0.0);
    if (w.x > 0.0) { r += (bones.mats[idx.x] * p) * w.x; }
    if (w.y > 0.0) { r += (bones.mats[idx.y] * p) * w.y; }
    if (w.z > 0.0) { r += (bones.mats[idx.z] * p) * w.z; }
    if (w.w > 0.0) { r += (bones.mats[idx.w] * p) * w.w; }
    return r;
}

fn skin_dir(v: vec3<f32>, idx: vec4<u32>, w_in: vec4<f32>) -> vec3<f32> {
    let w = normalize_weights(w_in);
    if (sum4(w) == 0.0) {
        return v;
    }

    var r = vec3<f32>(0.0);

    if (w.x > 0.0) {
        let m0 = mat3x3<f32>(bones.mats[idx.x][0].xyz, bones.mats[idx.x][1].xyz, bones.mats[idx.x][2].xyz);
        r += (m0 * v) * w.x;
    }
    if (w.y > 0.0) {
        let m1 = mat3x3<f32>(bones.mats[idx.y][0].xyz, bones.mats[idx.y][1].xyz, bones.mats[idx.y][2].xyz);
        r += (m1 * v) * w.y;
    }
    if (w.z > 0.0) {
        let m2 = mat3x3<f32>(bones.mats[idx.z][0].xyz, bones.mats[idx.z][1].xyz, bones.mats[idx.z][2].xyz);
        r += (m2 * v) * w.z;
    }
    if (w.w > 0.0) {
        let m3 = mat3x3<f32>(bones.mats[idx.w][0].xyz, bones.mats[idx.w][1].xyz, bones.mats[idx.w][2].xyz);
        r += (m3 * v) * w.w;
    }

    return normalize(r);
}

