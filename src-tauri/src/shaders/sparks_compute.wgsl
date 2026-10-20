// Particle Sparks Compute Simulation Shader
// Simulates 65,536 spark particles with exponential drag damping, gravity, and audio burst excitation.

struct Spark {
    pos: vec2<f32>,
    vel: vec2<f32>,
    age: f32,
    life: f32,
    hue: f32,
    size: f32,
};

struct SparksUniforms {
    dt: f32,
    time: f32,
    aspect: f32,
    max_sparks: u32,
    emit_rate: f32,
    speed: f32,
    spread: f32,
    gravity: f32,
    life_param: f32,
    size_param: f32,
    hue_cycle: f32,
    emit_count: u32,
    mouse_pos: vec2<f32>,
    mouse_vel: vec2<f32>,
    mouse_active: u32,
    mouse_down: u32,
    emit_mode: u32,          // 0: always/stream, 1: on_move, 2: on_press
    audio_beat: f32,
    emit_start: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var<uniform> u: SparksUniforms;
@group(0) @binding(1) var<storage, read_write> sparks: array<Spark>;

// PCG hash for fast GPU pseudo-random numbers
fn hash(seed: u32) -> u32 {
    var state = seed * 747796405u + 2891336453u;
    var word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn rand_f32(seed: u32) -> f32 {
    return f32(hash(seed)) / 4294967295.0;
}

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= u.max_sparks) {
        return;
    }

    var p = sparks[idx];
    let dt = u.dt;

    // Check if this particle index falls in the active emission batch for this frame
    var should_emit = false;
    if (u.emit_count > 0u) {
        let count = u.emit_count;
        let start = u.emit_start;
        let end = start + count;
        if (end <= u.max_sparks) {
            if (idx >= start && idx < end) {
                should_emit = true;
            }
        } else {
            // Circular buffer wrap
            if (idx >= start || idx < (end % u.max_sparks)) {
                should_emit = true;
            }
        }
    }

    if (should_emit) {
        // Emit a fresh spark
        let seed = idx * 1973u + u32(u.time * 1000.0) * 9277u;
        let r1 = rand_f32(seed);
        let r2 = rand_f32(seed + 101u);
        let r3 = rand_f32(seed + 203u);
        let r4 = rand_f32(seed + 307u);

        let angle = r1 * 6.2831853;
        let base_speed = u.speed * (0.25 + 0.75 * r2) * u.spread;
        var dir = vec2<f32>(cos(angle), sin(angle));

        var origin = u.mouse_pos;
        if (u.mouse_active == 0u) {
            origin = vec2<f32>(u.aspect * 0.5, 0.5);
        }

        // Add mouse motion momentum
        var initial_vel = dir * base_speed;
        if (u.mouse_active != 0u) {
            initial_vel += u.mouse_vel * 0.25;
        }

        // Audio burst excitation on emission
        if (u.audio_beat > 0.4) {
            initial_vel *= (1.0 + u.audio_beat * 1.5);
        }

        p.pos = origin;
        p.vel = initial_vel;
        p.age = 0.0;
        p.life = max(0.05, u.life_param * (0.4 + 0.6 * r3));
        p.hue = fract(u.time * u.hue_cycle + r4);
        p.size = u.size_param * (0.5 + 0.5 * r1);

        sparks[idx] = p;
        return;
    }

    // If particle has exceeded lifetime, keep it dormant
    if (p.age >= p.life) {
        return;
    }

    // Physical integration: exponential drag damping and gravity
    let drag = exp(-dt * 1.2);
    p.age += dt;
    p.vel *= drag;
    p.vel.y += u.gravity * dt;

    // Audio burst excitation: expanding shockwave boost for active sparks
    if (u.audio_beat > 0.7) {
        p.vel *= (1.0 + dt * u.audio_beat * 0.8);
    }

    p.pos += p.vel * dt;

    sparks[idx] = p;
}
