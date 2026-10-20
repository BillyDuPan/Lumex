// Floating Balls Compute Simulation Shader
// Simulates floating orbs with drift, cruising velocity damping, mouse forces, swell, and edge conditions.

struct Ball {
    pos: vec2<f32>,
    vel: vec2<f32>,
    angle: f32,
    phase: f32,
    speed_mult: f32,
    rand_val: f32,
    radius: f32,
    swell: f32,
    color_idx: u32,
    _pad: u32,
};

struct BallsUniforms {
    dt: f32,
    time: f32,
    aspect: f32,
    count: u32,
    speed: f32,
    drift: f32,
    size: f32,
    size_var: f32,
    pulse: f32,
    swell_param: f32,
    softness: f32,
    alpha: f32,
    mouse_mode: u32,         // 0: off, 1: repel, 2: attract, 3: swirl, 4: stir
    mouse_active: u32,
    mouse_pos: vec2<f32>,
    mouse_vel: vec2<f32>,
    mouse_radius: f32,
    mouse_force: f32,
    edge_mode: u32,          // 0: wrap, 1: bounce
    audio_bass: f32,
    audio_beat: f32,
    hue_cycle: f32,
};

@group(0) @binding(0) var<uniform> u: BallsUniforms;
@group(0) @binding(1) var<storage, read_write> balls: array<Ball>;

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= u.count) {
        return;
    }

    var b = balls[idx];
    let dt = u.dt;

    // Angular drift and cruising velocity
    b.angle += sin(u.time * 0.37 + b.phase) * u.drift * 0.9 * dt;
    let cruise = u.speed * 0.09 * b.speed_mult;
    let kv = 1.0 - exp(-dt * 0.6);
    let ks = 1.0 - exp(-dt * 6.0);

    b.vel.x += (cos(b.angle) * cruise - b.vel.x) * kv;
    b.vel.y += (sin(b.angle) * cruise - b.vel.y) * kv;

    var near = 0.0;
    if (u.mouse_active != 0u && u.mouse_mode != 0u) {
        let delta = b.pos - u.mouse_pos;
        let d = length(delta) + 1e-5;
        if (d < u.mouse_radius) {
            let q = 1.0 - d / u.mouse_radius;
            near = q * q * (3.0 - 2.0 * q);
            let n = delta / d;
            let F = u.mouse_force;

            if (u.mouse_mode == 1u) {
                // Repel: pushes away from cursor
                b.vel += n * (near * F * 5.0 * dt);
            } else if (u.mouse_mode == 2u) {
                // Attract: pulls toward cursor and damps
                b.vel -= n * (near * F * 4.0 * dt);
                b.vel *= max(0.0, 1.0 - near * dt * 2.5);
            } else if (u.mouse_mode == 3u) {
                // Swirl: tangential vortex around cursor
                b.vel += vec2<f32>(-n.y * 4.0 - n.x * 0.8, n.x * 4.0 - n.y * 0.8) * (near * F * dt);
            } else if (u.mouse_mode == 4u) {
                // Stir: drags balls along with mouse movement
                let k = min(1.0, near * F * dt * 4.0);
                b.vel += (u.mouse_vel * 0.8 - b.vel) * k;
            }
        }
    }

    // Velocity clamp
    let sp = length(b.vel);
    let vmax = 2.2;
    if (sp > vmax) {
        b.vel = (b.vel / sp) * vmax;
    }

    // Position integration
    b.pos += b.vel * dt;

    // Swell factor relaxation
    b.swell += (1.0 + u.swell_param * near - b.swell) * ks;

    // Radius with pulse and audio bass/beat modulation
    let audio_boost = 1.0 + u.audio_bass * 0.4 + u.audio_beat * 0.25;
    b.radius = u.size * (1.0 + u.size_var * (b.rand_val * 2.0 - 1.0)) * b.swell
             * (1.0 + u.pulse * sin(u.time * 0.8 + b.phase * 2.0)) * audio_boost;

    // Boundary conditions
    let m = b.radius * 1.1;
    if (u.edge_mode == 0u) {
        // Edge wrap
        if (b.pos.x < -m) {
            b.pos.x = u.aspect + m;
        } else if (b.pos.x > u.aspect + m) {
            b.pos.x = -m;
        }
        if (b.pos.y < -m) {
            b.pos.y = 1.0 + m;
        } else if (b.pos.y > 1.0 + m) {
            b.pos.y = -m;
        }
    } else {
        // Edge bounce
        if (b.pos.x < 0.0) {
            b.pos.x = 0.0;
            b.vel.x = abs(b.vel.x);
        } else if (b.pos.x > u.aspect) {
            b.pos.x = u.aspect;
            b.vel.x = -abs(b.vel.x);
        }
        if (b.pos.y < 0.0) {
            b.pos.y = 0.0;
            b.vel.y = abs(b.vel.y);
        } else if (b.pos.y > 1.0) {
            b.pos.y = 1.0;
            b.vel.y = -abs(b.vel.y);
        }
    }

    balls[idx] = b;
}
