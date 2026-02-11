struct ParticleSettings {
    position: vec4f,
    velocity: vec4f,
    acceleration: vec4f,
    color: vec4f,
    end_color: vec4f,
    // x=opacity, y=end_opacity, z=lifetime, w=duration
    emitter: vec4f,
    // x=spawn_rate, y=turbulence_strength, z=turbulence_scale, w=turbulence_speed
    emission: vec4f,
    // x=min, y=max
    lifetime_random: vec4f,
    // x=seed, y=particle_count, z=burst_count, w=looping
    counts: vec4u,
    position_random_min: vec4f,
    position_random_max: vec4f,
    velocity_random_min: vec4f,
    velocity_random_max: vec4f,
}

struct ParticleRuntime {
    // x=elapsed_time
    data: vec4f,
}

@group(1) @binding(0) var<uniform> particle: ParticleSettings;
@group(1) @binding(1) var<uniform> particle_runtime: ParticleRuntime;

struct VOut {
    @builtin(position) position: vec4f,
    @location(0) life_t: f32,
    @location(1) alive: f32,
}

fn hash11(n: f32) -> f32 {
    return fract(sin(n) * 43758.5453123);
}

fn hash31(n: f32) -> vec3f {
    return vec3f(
        hash11(n * 0.1031 + 0.11369),
        hash11(n * 0.11369 + 0.13787),
        hash11(n * 0.13787 + 0.17353),
    );
}

fn turbulence_field(p: vec3f, t: f32, seed: f32, scale: f32, speed: f32) -> vec3f {
    let frequency = max(scale, 0.0001);
    let phase = p * frequency + vec3f(t * speed + seed * 0.031);
    let tx = sin(phase.y * 1.7 + phase.z * 1.3 + seed * 0.11)
        + cos(phase.y * 2.3 - phase.z * 0.9 + seed * 0.07);
    let ty = sin(phase.z * 1.5 + phase.x * 1.1 + seed * 0.17)
        + cos(phase.z * 2.1 - phase.x * 0.8 + seed * 0.13);
    let tz = sin(phase.x * 1.9 + phase.y * 1.2 + seed * 0.19)
        + cos(phase.x * 2.5 - phase.y * 1.0 + seed * 0.23);
    return vec3f(tx, ty, tz) * 0.5;
}

fn spawn_time_for(idx: u32, burst_count: u32, spawn_rate: f32) -> f32 {
    if idx < burst_count {
        return 0.0;
    }
    if spawn_rate <= 0.0 {
        return 1e30;
    }
    return f32(idx - burst_count) / spawn_rate;
}

fn emission_time(total_time: f32, duration: f32, looping: bool) -> f32 {
    if duration <= 0.0 {
        return total_time;
    }
    if looping {
        return total_time - floor(total_time / duration) * duration;
    }
    return min(total_time, duration);
}

@vertex
fn vs_main(@builtin(instance_index) idx: u32) -> VOut {
    var out: VOut;
    out.life_t = 0.0;
    out.alive = 0.0;
    out.position = vec4f(0.0, 0.0, 2.0, 1.0);

    let seed = f32(particle.counts.x);
    let particle_count = max(particle.counts.y, 1u);
    let burst_count = particle.counts.z;
    let looping = particle.counts.w != 0u;

    let total_time = max(particle_runtime.data.x, 0.0);
    let duration = max(particle.emitter.w, 0.0);
    let emit_time = emission_time(total_time, duration, looping);

    let spawn_rate = particle.emission.x;
    let spawn_time = spawn_time_for(idx, burst_count, spawn_rate);
    let duration_limited = duration > 0.0 && spawn_time > duration;
    if duration_limited || emit_time < spawn_time {
        return out;
    }

    let r0 = hash31(f32(idx) + seed * 17.0);
    let r1 = hash31(f32(idx) + seed * 29.0 + 13.0);

    let lifetime_mul = mix(
        particle.lifetime_random.x,
        particle.lifetime_random.y,
        hash11(f32(idx) * 0.791 + seed * 0.173),
    );
    let base_lifetime = max(particle.emitter.z, 0.0001);
    let lifetime = max(base_lifetime * max(lifetime_mul, 0.0001), 0.0001);

    let age = max(emit_time - spawn_time, 0.0);
    if age > lifetime {
        return out;
    }
    let life_t = clamp(age / lifetime, 0.0, 1.0);

    let spawn_pos = particle.position.xyz + mix(
        particle.position_random_min.xyz,
        particle.position_random_max.xyz,
        r0,
    );
    let velocity = particle.velocity.xyz + mix(
        particle.velocity_random_min.xyz,
        particle.velocity_random_max.xyz,
        r1,
    );

    let ballistic =
        spawn_pos +
        velocity * age +
        0.5 * particle.acceleration.xyz * age * age;

    let idx_norm = f32(idx) / f32(particle_count);
    let turbulence = turbulence_field(
        ballistic + r1 * 2.0,
        age + idx_norm * 0.5,
        seed,
        particle.emission.z,
        particle.emission.w,
    );
    let world_position = ballistic + turbulence * particle.emission.y;

    out.position = camera.view_proj_mat * vec4f(world_position, 1.0);
    out.life_t = life_t;
    out.alive = 1.0;
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4f {
    if in.alive < 0.5 {
        discard;
    }

    let color = mix(particle.color.rgb, particle.end_color.rgb, in.life_t);
    let opacity = mix(particle.emitter.x, particle.emitter.y, in.life_t);
    if opacity <= 0.001 {
        discard;
    }
    return vec4f(color, opacity);
}
