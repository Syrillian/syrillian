#use default_vertex
#use model

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
