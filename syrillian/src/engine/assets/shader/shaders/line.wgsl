struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) p_px: vec2<f32>,   // interpolated pixel position for this fragment
    @location(1) a_px: vec2<f32>,   // line start
    @location(2) b_px: vec2<f32>,   // line end
    @location(3) half_w: f32,       // half thickness in px
    @location(4) color: vec4<f32>,  // rgba
};

struct PushConstants {
    a_px: vec2<f32>,
    b_px: vec2<f32>,
    a_color: vec4<f32>,
    b_color: vec4<f32>,
    thickness_px: f32,
};

var<immediate> pc: PushConstants;

fn to_ndc(px: vec2<f32>) -> vec4<f32> {
    let screen = vec2<f32>(system.screen);
    let ndc = vec2<f32>(
        (px.x / screen.x) * 2.0 - 1.0,
        1.0 - (px.y / screen.y) * 2.0
    );
    return vec4<f32>(ndc, 0.0, 1.0);
}

fn corner_from_vid(vid: u32, c0: vec2<f32>, c1: vec2<f32>, c2: vec2<f32>, c3: vec2<f32>) -> vec2<f32> {
    switch(vid) {
        case 0u: { return c0; }
        case 1u: { return c1; }
        case 2u: { return c2; }
        case 3u: { return c0; }
        case 4u: { return c2; }
        default: { return c3; }
    }
}

fn color_from_vid(vid: u32) -> vec4<f32> {
    switch(vid) {
        case 0u: { return pc.a_color; }
        case 1u: { return pc.a_color; }
        case 2u: { return pc.b_color; }
        case 3u: { return pc.a_color; }
        case 4u: { return pc.b_color; }
        default: { return pc.b_color; }
    }
}

@vertex
fn ui_line_vs(@builtin(vertex_index) vid: u32) -> VOut {
    var out: VOut;

    let a = pc.a_px;
    let b = pc.b_px;

    let v = b - a;
    let len = length(v);

    let dir = select(vec2<f32>(1.0, 0.0), v / len, len > 1e-5);
    let n = vec2<f32>(-dir.y, dir.x);

    let half_w = max(pc.thickness_px * 0.5, 0.5);

    let aa_pad = 1.5;
    let R = half_w + aa_pad;

    let a2 = a - dir * R;
    let b2 = b + dir * R;

    let c0 = a2 - n * R;
    let c1 = a2 + n * R;
    let c2 = b2 + n * R;
    let c3 = b2 - n * R;

    let p = corner_from_vid(vid, c0, c1, c2, c3);
    let color = color_from_vid(vid);

    out.position = to_ndc(p);
    out.p_px = p;

    out.a_px = a;
    out.b_px = b;
    out.half_w = half_w;
    out.color = color;

    return out;
}

fn dist_to_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let ab_len2 = dot(ab, ab);
    if (ab_len2 <= 1e-8) {
        return length(p - a);
    }
    let t = clamp(dot(p - a, ab) / ab_len2, 0.0, 1.0);
    let q = a + t * ab;
    return length(p - q);
}

@fragment
fn ui_line_fs(in: VOut) -> @location(0) vec4<f32> {
    let d = dist_to_segment(in.p_px, in.a_px, in.b_px) - in.half_w;
    let w = max(fwidth(d), 1e-3);
    let alpha = smoothstep(w, -w, d) * in.color.a;
    if (alpha <= 1e-4) { discard; }

    return vec4<f32>(in.color.rgb, alpha);
}