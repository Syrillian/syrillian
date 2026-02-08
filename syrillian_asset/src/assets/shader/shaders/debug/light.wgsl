#use light

struct VSOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

struct PushConstants {
    light_index: u32,
};

var<immediate> pc: PushConstants;

// Sun Light

fn calculate_sun_offset(light: Light, vid: u32, iid: u32) -> vec3<f32> {
    const ROWS: u32 = 3;
    const COLS: u32 = 3;

    let x = f32(iid % ROWS) - f32(ROWS / 2);
    let y = f32(iid / COLS) - f32(COLS / 2);

    let dir = normalize(light.direction);
    let dirT = cross(dir, vec3(0.0, 1.0, 0.0));
    let dirB = cross(dir, dirT);

    var offset = dirT * x + dirB * y;
    if vid == 0 {
        offset += dir * light.range;
    }

    return offset;
}

// Point Light

fn calculate_point_offset(light: Light, vid: u32, iid: u32) -> vec3<f32> {
    let ray_dir = calculate_point_dir(iid);
    let scaled = ray_dir * light.range / 2;

    if vid == 0 {
        return scaled;
    } else {
        return -scaled;
    }
}

fn calculate_point_dir(iid: u32) -> vec3<f32> {
    if iid > 5 {
        return vec3(0.0, 1.0, 0.0);
    }

    const DIRS = array<vec3<f32>, 6>(
        vec3(1.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        vec3(0.0, 0.0, 1.0),

        vec3(0.5, 0.5, 0.0),
        vec3(0.0, 0.5, 0.5),
        vec3(0.5, 0.0, 0.5),
    );

    return DIRS[iid];
}

// Spot Light
fn calculate_spot_offset(light: Light, vid: u32, iid: u32) -> vec3<f32> {
    let dir = normalize(light.direction);

    var up = vec3(0.0, 1.0, 0.0);
    if abs(dot(dir, up)) > 0.99 {
        up = vec3(1.0, 0.0, 0.0);
    }
    let T = normalize(cross(dir, up));
    let B = cross(dir, T);

    // Treat angles as radians; keep them sane.
    let inner = min(light.inner_angle, light.outer_angle);
    let outer = max(light.inner_angle, light.outer_angle);

    // Center ray
    let r_in  = tan(inner) * light.range;
    let r_out = tan(outer) * light.range;

    // Instance mapping (expect exactly 9 instances for SPOT):
    // 0   -> center ray along `dir`
    // 1..4 -> inner-cone cardinals: +T, -T, +B, -B
    // 5..8 -> outer-cone cardinals: +T, -T, +B, -B

    // Choose ring and axis for cardinals
    let use_outer = iid >= 5u;
    let k = select(iid - 1u, iid - 5u, use_outer); // 0..3 expected

    var axis: vec3<f32>;
    if k == 0u { axis =  T; }
    else if k == 1u { axis = -T; }
    else if k == 2u { axis =  B; }
    else            { axis = -B; }

    let r = select(r_in, r_out, use_outer);
    let tip = dir * light.range + axis * r;

    if vid == 0u { return tip; }
    return vec3(0.0, 0.0, 0.0);
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, @builtin(instance_index) iid: u32) -> VSOut {
    var out: VSOut;
    let light = lights[pc.light_index];

    var offset: vec3<f32>;
    var alpha: f32;

    if light.type_id == LIGHT_TYPE_SUN {
        offset = calculate_sun_offset(light, vid, iid);
        alpha = f32(vid) / 2.;
    } else if light.type_id == LIGHT_TYPE_POINT {
        offset = calculate_point_offset(light, vid, iid);
        alpha = 0.5;
    } else if light.type_id == LIGHT_TYPE_SPOT {
        offset = calculate_spot_offset(light, vid, iid);
        alpha = f32(vid) / 2.;
    }

    out.position = vec4(light.position + offset, 1.0);
    out.color = vec4(light.color, alpha);

    out.position = camera.view_proj_mat * out.position;

    return out;
}

@fragment
fn fs_main(in: VSOut) -> @location(0) vec4<f32> {
    return in.color;
}
