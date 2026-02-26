struct CameraData {
    position: vec3<f32>,
    fov: f32,
    view_mat: mat4x4<f32>,
    projection_mat: mat4x4<f32>,
    view_proj_mat: mat4x4<f32>,
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

struct ParticleSettings {
    position: vec4f,
    velocity: vec4f,
    acceleration: vec4f,
    color: vec4f,
    end_color: vec4f,
    emitter: vec4f,
    emission: vec4f,
    lifetime_random: vec4f,
    counts: vec4u,
    position_random_min: vec4f,
    position_random_max: vec4f,
    velocity_random_min: vec4f,
    velocity_random_max: vec4f,
}

struct ParticleRuntime {
    data: vec4f,
}

@group(0) @binding(0) var<uniform> camera: CameraData;
@group(0) @binding(1) var<uniform> system: SystemData;
@group(1) @binding(0) var<uniform> particle: ParticleSettings;
@group(1) @binding(1) var<uniform> particle_runtime: ParticleRuntime;

struct VIn {
    @location(0) world_pos_alive: vec4f,
    @location(1) life_t: f32,
}

struct VOut {
    @builtin(position) position: vec4f,
    @location(0) life_t: f32,
    @location(1) alive: f32,
}

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    out.life_t = in.life_t;
    out.alive = in.world_pos_alive.w;
    out.position = camera.view_proj_mat * vec4f(in.world_pos_alive.xyz, 1.0);
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4f {
    if (in.alive < 0.5) {
        discard;
    }

    let color = mix(particle.color.rgb, particle.end_color.rgb, in.life_t);
    let opacity = mix(particle.emitter.x, particle.emitter.y, in.life_t);
    if (opacity <= 0.001) {
        discard;
    }

    return vec4f(color, opacity);
}
