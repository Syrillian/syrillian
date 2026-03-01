struct VInput {
    @location(0) position: vec3<f32>,
}

struct FInput {
    @builtin(position) clip: vec4<f32>,
    @location(0) position:   vec3<f32>,
}

struct FOutput {
      @location(0) out_color: vec4<f32>,
}
