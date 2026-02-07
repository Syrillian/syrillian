fn sum4(v: vec4<f32>) -> f32 {
    return v.x + v.y + v.z + v.w;
}

fn safe_rsqrt(x: f32) -> f32 { return inverseSqrt(max(x, 1e-8)); }
fn safe_normalize(v: vec3<f32>) -> vec3<f32> { return v * safe_rsqrt(dot(v, v)); }

fn oct_encode(n: vec3<f32>) -> vec2<f32> {
    let denom = max(abs(n.x) + abs(n.y) + abs(n.z), 1e-6);
    var v = n / denom;
    var enc = v.xy;
    if (v.z < 0.0) {
        enc = (1.0 - abs(enc.yx)) * sign(enc);
    }
    return enc;
}

