const MAX_BONES: u32 = 256u;
const WORDS_PER_VERTEX: u32 = 17u;

struct BoneData {
    mats: array<mat4x4<f32>, MAX_BONES>,
}

struct SkinningParams {
    vertex_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var<uniform> bones: BoneData;
@group(0) @binding(1) var<uniform> params: SkinningParams;
@group(0) @binding(2) var<storage, read> src_words: array<u32>;
@group(0) @binding(3) var<storage, read_write> dst_position_words: array<u32>;
@group(0) @binding(4) var<storage, read_write> dst_normal_words: array<u32>;
@group(0) @binding(5) var<storage, read_write> dst_tangent_words: array<u32>;

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
    if (w.x > 0.0) {
        r += (bones.mats[idx.x] * p) * w.x;
    }
    if (w.y > 0.0) {
        r += (bones.mats[idx.y] * p) * w.y;
    }
    if (w.z > 0.0) {
        r += (bones.mats[idx.z] * p) * w.z;
    }
    if (w.w > 0.0) {
        r += (bones.mats[idx.w] * p) * w.w;
    }
    return r;
}

fn skin_dir(v: vec3<f32>, idx: vec4<u32>, w_in: vec4<f32>) -> vec3<f32> {
    let w = normalize_weights(w_in);
    if (sum4(w) == 0.0) {
        return v;
    }

    var r = vec3<f32>(0.0);

    if (w.x > 0.0) {
        let m0 = mat3x3<f32>(
            bones.mats[idx.x][0].xyz,
            bones.mats[idx.x][1].xyz,
            bones.mats[idx.x][2].xyz
        );
        r += (m0 * v) * w.x;
    }
    if (w.y > 0.0) {
        let m1 = mat3x3<f32>(
            bones.mats[idx.y][0].xyz,
            bones.mats[idx.y][1].xyz,
            bones.mats[idx.y][2].xyz
        );
        r += (m1 * v) * w.y;
    }
    if (w.z > 0.0) {
        let m2 = mat3x3<f32>(
            bones.mats[idx.z][0].xyz,
            bones.mats[idx.z][1].xyz,
            bones.mats[idx.z][2].xyz
        );
        r += (m2 * v) * w.z;
    }
    if (w.w > 0.0) {
        let m3 = mat3x3<f32>(
            bones.mats[idx.w][0].xyz,
            bones.mats[idx.w][1].xyz,
            bones.mats[idx.w][2].xyz
        );
        r += (m3 * v) * w.w;
    }

    return normalize(r);
}

fn load_f32(word_index: u32) -> f32 {
    return bitcast<f32>(src_words[word_index]);
}

fn load_vec3(base: u32) -> vec3<f32> {
    return vec3<f32>(
        load_f32(base),
        load_f32(base + 1u),
        load_f32(base + 2u)
    );
}

fn load_vec4(base: u32) -> vec4<f32> {
    return vec4<f32>(
        load_f32(base),
        load_f32(base + 1u),
        load_f32(base + 2u),
        load_f32(base + 3u)
    );
}

fn load_uvec4(base: u32) -> vec4<u32> {
    return vec4<u32>(
        src_words[base],
        src_words[base + 1u],
        src_words[base + 2u],
        src_words[base + 3u]
    );
}

fn load_packed_u16x2(word_index: u32) -> vec2<u32> {
    let word = src_words[word_index];
    return vec2<u32>(
        word & 0xFFFFu,
        (word >> 16u) & 0xFFFFu
    );
}

fn load_u16x4(base: u32) -> vec4<u32> {
    let lo = load_packed_u16x2(base);
    let hi = load_packed_u16x2(base + 1u);
    return vec4<u32>(lo.x, lo.y, hi.x, hi.y);
}

@compute @workgroup_size(64, 1, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.vertex_count) {
        return;
    }

    let base = idx * WORDS_PER_VERTEX;

    let p_obj = vec4<f32>(load_vec3(base + 0u), 1.0);
    let n_obj = load_vec3(base + 5u);
    let t_obj = load_vec3(base + 8u);
    let bone_idx = load_u16x4(base + 11u);
    let bone_w = load_vec4(base + 13u);

    let p_sk = skin_pos(p_obj, bone_idx, bone_w);
    let n_sk = skin_dir(n_obj, bone_idx, bone_w);
    let t_sk = skin_dir(t_obj, bone_idx, bone_w);

    let out_base = idx * 3u;

    dst_position_words[out_base] = bitcast<u32>(p_sk.x);
    dst_position_words[out_base + 1u] = bitcast<u32>(p_sk.y);
    dst_position_words[out_base + 2u] = bitcast<u32>(p_sk.z);

    dst_normal_words[out_base] = bitcast<u32>(n_sk.x);
    dst_normal_words[out_base + 1u] = bitcast<u32>(n_sk.y);
    dst_normal_words[out_base + 2u] = bitcast<u32>(n_sk.z);

    dst_tangent_words[out_base] = bitcast<u32>(t_sk.x);
    dst_tangent_words[out_base + 1u] = bitcast<u32>(t_sk.y);
    dst_tangent_words[out_base + 2u] = bitcast<u32>(t_sk.z);
}
