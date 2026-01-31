struct CameraData {
    position:       vec3<f32>,
    fov: f32,
    view_mat:       mat4x4<f32>,
    projection_mat: mat4x4<f32>,
    view_proj_mat:  mat4x4<f32>,
    inv_view_proj_mat: mat4x4<f32>,
    near: f32,
    far: f32,
    fov_target: f32,
    zoom_speed: f32,
}

struct SystemData {
    screen: vec2<u32>,
    time: f32,
    delta_time: f32,
}

@group(0) @binding(0) var<uniform> camera: CameraData;
@group(0) @binding(1) var<uniform> system: SystemData;
