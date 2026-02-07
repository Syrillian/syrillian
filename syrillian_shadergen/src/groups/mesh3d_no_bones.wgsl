struct VInput {
    @location(0) position: vec3<f32>,
    @location(1) uv:       vec2<f32>,
    @location(2) normal:   vec3<f32>,
    @location(3) tangent:  vec3<f32>,
}

struct FInput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv:         vec2<f32>,
    @location(1) position:   vec3<f32>,
    @location(2) normal:     vec3<f32>,
    @location(3) tangent:    vec3<f32>,
    @location(4) bitangent:  vec3<f32>,
}

struct FOutput {
      @location(0) out_color    : vec4<f32>,
      @location(1) out_normal   : vec4<f32>,
      @location(2) out_material : vec4<f32>,
}
