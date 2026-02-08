#use default_vertex
#use model

struct PickColor {
    color: vec4<f32>,
};

var<immediate> pick: PickColor;

@fragment
fn fs_main(in: FInput) -> @location(0) vec4<f32> {
    return pick.color;
}
