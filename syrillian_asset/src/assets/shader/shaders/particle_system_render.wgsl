#use render
#use particle_render

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
