// Craig Reynolds Intelligent Life Boids Compute Simulation Shader
// Simulates 10,000 schooling boid agents with toroidal wrapping, Separation, Alignment, Cohesion,
// Spore foraging, and predator mouse reactions (hunt, flee, orbit).

struct Boid {
    pos: vec2<f32>,
    vel: vec2<f32>,
    target_pos: vec2<f32>,
    light: f32,
    energy: f32,
    species: u32,
    phase: f32,
    _pad: vec2<f32>,
};

struct Spore {
    pos: vec2<f32>,
    vel: vec2<f32>,
    pulse: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

struct BoidsUniforms {
    dt: f32,
    time: f32,
    aspect: f32,
    boid_count: u32,
    spore_count: u32,
    speed: f32,
    flock: f32,
    separation: f32,
    sense: f32,
    biolum: f32,
    mouse_mode: u32,         // 0: hunt, 1: flee, 2: orbit, 3: off
    mouse_active: u32,
    mouse_pos: vec2<f32>,
    mouse_radius: f32,
    mouse_force: f32,
    edge_mode: u32,          // 0: wrap, 1: bounce
    life_type: u32,          // 0: swimmer, 1: microbe, 2: spore
    audio_beat: f32,
    max_neighbors: u32,
};

@group(0) @binding(0) var<uniform> u: BoidsUniforms;
@group(0) @binding(1) var<storage, read_write> boids: array<Boid>;
@group(0) @binding(2) var<storage, read_write> spores: array<Spore>;

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let i = global_id.x;
    if (i >= u.boid_count) {
        return;
    }

    var o = boids[i];
    let dt = u.dt;
    let A = u.aspect;

    // Bioluminescent pulse decay and oscillation phase
    o.phase += dt * (3.0 + u.speed * 5.0);
    o.light = max(0.15, o.light - dt * 0.8);

    let sense = u.sense;
    let sense2 = sense * sense;
    let sepDist = sense * 0.38;
    let sepDist2 = sepDist * sepDist;

    var sep = vec2<f32>(0.0);
    var ali = vec2<f32>(0.0);
    var coh = vec2<f32>(0.0);
    var sepN = 0.0;
    var aliN = 0.0;
    var cohN = 0.0;

    // Neighbor sampling loop with configurable cap for high-density performance
    let max_n = min(u.boid_count, u.max_neighbors);
    let stride = max(1u, u.boid_count / max_n);

    for (var n = 0u; n < max_n; n++) {
        let j = (i + n * stride + 1u) % u.boid_count;
        if (i == j) {
            continue;
        }

        let other = boids[j];
        var delta = other.pos - o.pos;

        // Toroidal distance wrapping
        if (u.edge_mode == 0u) {
            if (delta.x > A * 0.5) {
                delta.x -= A;
            } else if (delta.x < -A * 0.5) {
                delta.x += A;
            }
            if (delta.y > 0.5) {
                delta.y -= 1.0;
            } else if (delta.y < -0.5) {
                delta.y += 1.0;
            }
        }

        let d2 = dot(delta, delta);
        if (d2 < sense2 && d2 > 1e-6) {
            let d = sqrt(d2);
            // 1. Separation: Reynolds repulsion when inside sepDist
            if (d2 < sepDist2) {
                let f = (1.0 - d / sepDist) / d;
                sep -= delta * f;
                sepN += 1.0;
            }
            // 2. Alignment: match neighbor velocities
            ali += other.vel;
            aliN += 1.0;
            // 3. Cohesion: steer toward center of mass
            coh += (o.pos + delta);
            cohN += 1.0;
        }
    }

    var acc = vec2<f32>(0.0);
    if (sepN > 0.0) {
        acc += (sep / sepN) * (u.separation * 3.5);
    }
    if (aliN > 0.0) {
        acc += ((ali / aliN) - o.vel) * (u.flock * 1.5);
    }
    if (cohN > 0.0) {
        acc += ((coh / cohN) - o.pos) * (u.flock * 1.8);
    }

    // Food spore foraging
    var closestDist2 = sense2 * 2.0;
    var foodDir = vec2<f32>(0.0);
    var foundFood = false;

    for (var s = 0u; s < u.spore_count; s++) {
        var sdelta = spores[s].pos - o.pos;
        if (u.edge_mode == 0u) {
            if (sdelta.x > A * 0.5) {
                sdelta.x -= A;
            } else if (sdelta.x < -A * 0.5) {
                sdelta.x += A;
            }
            if (sdelta.y > 0.5) {
                sdelta.y -= 1.0;
            } else if (sdelta.y < -0.5) {
                sdelta.y += 1.0;
            }
        }

        let sd2 = dot(sdelta, sdelta);
        if (sd2 < closestDist2) {
            closestDist2 = sd2;
            foodDir = sdelta;
            foundFood = true;
        }
    }

    if (foundFood) {
        let dist = sqrt(closestDist2);
        if (dist < 0.025) {
            // Feeding excitation flashes bioluminescent light to 1.0
            o.light = 1.0;
        } else {
            let foodPull = min(2.5, 0.08 / (dist + 0.01));
            acc += (foodDir / dist) * foodPull;
        }
    }

    // Predator mouse reactions: hunt, flee, orbit
    if (u.mouse_active != 0u && u.mouse_mode != 3u) {
        var mdelta = u.mouse_pos - o.pos;
        if (u.edge_mode == 0u) {
            if (mdelta.x > A * 0.5) {
                mdelta.x -= A;
            } else if (mdelta.x < -A * 0.5) {
                mdelta.x += A;
            }
            if (mdelta.y > 0.5) {
                mdelta.y -= 1.0;
            } else if (mdelta.y < -0.5) {
                mdelta.y += 1.0;
            }
        }

        let md2 = dot(mdelta, mdelta);
        if (md2 < u.mouse_radius * u.mouse_radius && md2 > 1e-6) {
            let md = sqrt(md2);
            let mn = mdelta / md;
            let str = (1.0 - md / u.mouse_radius) * u.mouse_force * 4.0;

            if (u.mouse_mode == 0u) {
                // Hunt: accelerate toward cursor
                acc += mn * (str * 2.5);
                o.light = max(o.light, str * 0.7);
            } else if (u.mouse_mode == 1u) {
                // Flee: scatter away from cursor
                acc -= mn * (str * 3.5);
            } else if (u.mouse_mode == 2u) {
                // Orbit: tangential swirl
                acc += vec2<f32>(-mn.y * 1.5 + mn.x * 0.4, mn.x * 1.5 + mn.y * 0.4) * str;
            }
        }
    }

    // Wandering natural drift
    let wander = sin(u.time * 2.5 + o.phase) * 0.6;
    let heading = atan2(o.vel.y, o.vel.x);
    acc += vec2<f32>(-sin(heading) * wander, cos(heading) * wander);

    // Audio beat excitation
    if (u.audio_beat > 0.5) {
        o.light = max(o.light, u.audio_beat);
    }

    // Steering and velocity update
    let steerForce = 4.0;
    o.vel += acc * (dt * steerForce);

    let maxSpeed = u.speed * 0.28;
    let spd = length(o.vel);
    let targetSpd = maxSpeed * max(0.5, o.energy);
    if (spd > 1e-5) {
        let smoothSpd = spd + (targetSpd - spd) * (1.0 - exp(-dt * 3.0));
        o.vel = (o.vel / spd) * smoothSpd;
    }

    o.pos += o.vel * dt;

    // Boundaries
    if (u.edge_mode == 1u) {
        // Bounce
        if (o.pos.x < 0.0) {
            o.pos.x = 0.0;
            o.vel.x = abs(o.vel.x);
        } else if (o.pos.x > A) {
            o.pos.x = A;
            o.vel.x = -abs(o.vel.x);
        }
        if (o.pos.y < 0.0) {
            o.pos.y = 0.0;
            o.vel.y = abs(o.vel.y);
        } else if (o.pos.y > 1.0) {
            o.pos.y = 1.0;
            o.vel.y = -abs(o.vel.y);
        }
    } else {
        // Toroidal wrap
        if (o.pos.x < 0.0) {
            o.pos.x += A;
        } else if (o.pos.x > A) {
            o.pos.x -= A;
        }
        if (o.pos.y < 0.0) {
            o.pos.y += 1.0;
        } else if (o.pos.y > 1.0) {
            o.pos.y -= 1.0;
        }
    }

    o.target_pos = o.vel;
    boids[i] = o;
}
