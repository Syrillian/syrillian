#use model
#use material
#use material_textures

struct VInput {
    @location(0) position: vec3f,
    @location(1) uv: vec2f,
}

struct FInput {
    @builtin(position) clip: vec4f,
    @location(0) uv: vec2f,
}

struct PickColor {
    color: vec4<f32>,
};

var<immediate> pick: PickColor;

@vertex
fn vs_main_2d(in: VInput) -> FInput {
    var out: FInput;

    out.clip = model.transform * vec4<f32>(in.position, 1.0);
    out.uv = vec2<f32>(in.uv.x, 1.0 - in.uv.y);

    return out;
}


@fragment
fn fs_main(in: FInput) -> @location(0) vec4<f32> {
    return pick.color;
}
