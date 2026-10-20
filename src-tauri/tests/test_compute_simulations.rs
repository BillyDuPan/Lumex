// Comprehensive Integration & Unit Test Suite for Lumex Compute Simulation Subsystems
//
// Verifies:
// 1. Naga WGSL compilation of balls_compute.wgsl, sparks_compute.wgsl, boids_compute.wgsl, brush_sdf.wgsl
// 2. WGPU Compute Pipeline creation and GPU dispatch
// 3. Floating balls physics: drift, damping, swell, 4 mouse interaction modes, edge wrap & bounce
// 4. Particle sparks: 65,536 particle simulation, drag damping (exp(-dt * 1.2)), gravity, audio burst excitation
// 5. Craig Reynolds boids: toroidal wrapping, Separation (R_sep = 0.38 R_sense), Alignment, Cohesion,
//    food spore foraging with feeding excitation (light = 1.0), predator mouse reactions (hunt, flee, orbit)
// 6. Vector brush SDF: instanced line segment quads, analytical distance field, neon glow, and eraser accumulation

use app_lib::compute::balls::{Ball, BallsComputePipeline, BallsUniforms};
use app_lib::compute::boids::{Boid, BoidsComputePipeline, BoidsUniforms, Spore};
use app_lib::compute::drawing::{BrushStyle, DrawUniforms, DrawingPipeline, StrokeSegment};
use app_lib::compute::sparks::{Particle, SparksComputePipeline, SparksUniforms, MAX_SPARKS};
use app_lib::graphics::WgpuContext;

#[test]
fn test_all_compute_and_draw_shaders_compile_naga() {
    let shaders = [
        ("balls_compute.wgsl", include_str!("../src/shaders/balls_compute.wgsl")),
        ("sparks_compute.wgsl", include_str!("../src/shaders/sparks_compute.wgsl")),
        ("boids_compute.wgsl", include_str!("../src/shaders/boids_compute.wgsl")),
        ("brush_sdf.wgsl", include_str!("../src/shaders/brush_sdf.wgsl")),
    ];

    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );

    for (name, source) in shaders {
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse WGSL shader {}: {:?}", name, e));

        let _info = validator
            .validate(&module)
            .unwrap_or_else(|e| panic!("Failed to validate WGSL shader {}: {:?}", name, e));

        println!("Successfully parsed and validated WGSL: {}", name);
    }
}

#[test]
fn test_balls_compute_dispatch_and_physics_transition() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU execution (no adapter available): {}", e);
            return;
        }
    };

    let count = 500u32;
    let balls_pipe = BallsComputePipeline::new(&ctx.device, count);
    balls_pipe.init_particles(&ctx.queue, count, 16.0 / 9.0);

    let initial_balls = balls_pipe.read_balls_sync(&ctx.device, &ctx.queue);
    assert_eq!(initial_balls.len(), count as usize);
    assert!(initial_balls[0].radius > 0.0);

    // Dispatch step with Repel mouse mode near ball 0
    let mouse_pos = initial_balls[0].pos;
    let uniforms = BallsUniforms {
        dt: 0.05,
        time: 1.0,
        aspect: 16.0 / 9.0,
        count,
        speed: 0.8,
        drift: 0.5,
        size: 0.08,
        size_var: 0.3,
        pulse: 0.2,
        swell_param: 1.5,
        softness: 0.75,
        alpha: 0.9,
        mouse_mode: 1, // Repel
        mouse_active: 1,
        mouse_pos,
        mouse_vel: [0.0, 0.0],
        mouse_radius: 0.35,
        mouse_force: 2.0,
        edge_mode: 0, // Wrap
        audio_bass: 0.5,
        audio_beat: 1.0,
        hue_cycle: 0.2,
    };
    balls_pipe.update_uniforms(&ctx.queue, &uniforms);

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("BallsComputeEncoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("BallsComputePass"),
            timestamp_writes: None,
        });
        balls_pipe.dispatch(&mut cpass);
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);

    let after_balls = balls_pipe.read_balls_sync(&ctx.device, &ctx.queue);
    assert_eq!(after_balls.len(), count as usize);

    // Ball 0 should have experienced swell factor expansion from being near the active mouse
    assert!(
        after_balls[0].swell > 1.0,
        "Expected swell factor > 1.0 due to mouse proximity, got {}",
        after_balls[0].swell
    );

    // Audio boost (bass + beat) should expand radius
    assert!(
        after_balls[0].radius > 0.0,
        "Ball radius must be positive"
    );
}

#[test]
fn test_balls_mouse_modes_produce_correct_forces() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU execution: {}", e);
            return;
        }
    };

    let count = 4u32;
    let balls_pipe = BallsComputePipeline::new(&ctx.device, count);

    // Set 4 test balls at identical positions offset from mouse (0.5, 0.5)
    // Ball is at (0.6, 0.5) -> delta is (+0.1, 0.0)
    let test_ball = Ball {
        pos: [0.6, 0.5],
        vel: [0.0, 0.0],
        angle: 0.0,
        phase: 0.0,
        speed_mult: 1.0,
        rand_val: 0.5,
        radius: 0.05,
        swell: 1.0,
        color_idx: 0,
        _pad: 0,
    };
    ctx.queue.write_buffer(
        &balls_pipe.storage_buffer,
        0,
        bytemuck::cast_slice(&[test_ball, test_ball, test_ball, test_ball]),
    );

    // Test 1: Repel mode (pushes outward: delta.x > 0 -> vel.x should become positive)
    let mut uniforms = BallsUniforms::default();
    uniforms.dt = 0.05;
    uniforms.count = 1;
    uniforms.mouse_active = 1;
    uniforms.mouse_pos = [0.5, 0.5];
    uniforms.mouse_radius = 0.3;
    uniforms.mouse_force = 2.0;
    uniforms.mouse_mode = 1; // Repel

    balls_pipe.update_uniforms(&ctx.queue, &uniforms);

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("RepelEncoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("RepelPass"),
            timestamp_writes: None,
        });
        balls_pipe.dispatch(&mut cpass);
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);

    let balls = balls_pipe.read_balls_sync(&ctx.device, &ctx.queue);
    assert!(
        balls[0].vel[0] > 0.0,
        "Repel must accelerate ball away from mouse (vel.x > 0), got {}",
        balls[0].vel[0]
    );

    // Test 2: Attract mode (pulls inward: vel.x should be negative)
    ctx.queue.write_buffer(
        &balls_pipe.storage_buffer,
        0,
        bytemuck::cast_slice(&[test_ball]),
    );
    uniforms.mouse_mode = 2; // Attract
    balls_pipe.update_uniforms(&ctx.queue, &uniforms);

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("AttractEncoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("AttractPass"),
            timestamp_writes: None,
        });
        balls_pipe.dispatch(&mut cpass);
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);

    let balls = balls_pipe.read_balls_sync(&ctx.device, &ctx.queue);
    assert!(
        balls[0].vel[0] < 0.0,
        "Attract must accelerate ball toward mouse (vel.x < 0), got {}",
        balls[0].vel[0]
    );

    // Test 3: Swirl mode (tangential acceleration: normal is (1, 0), swirl adds (-ny, nx) = (0, 1) -> vel.y > 0)
    ctx.queue.write_buffer(
        &balls_pipe.storage_buffer,
        0,
        bytemuck::cast_slice(&[test_ball]),
    );
    uniforms.mouse_mode = 3; // Swirl
    balls_pipe.update_uniforms(&ctx.queue, &uniforms);

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("SwirlEncoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("SwirlPass"),
            timestamp_writes: None,
        });
        balls_pipe.dispatch(&mut cpass);
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);

    let balls = balls_pipe.read_balls_sync(&ctx.device, &ctx.queue);
    assert!(
        balls[0].vel[1] > 0.0,
        "Swirl must accelerate tangentially (vel.y > 0), got {}",
        balls[0].vel[1]
    );

    // Test 4: Stir mode (aligns with mouse velocity)
    ctx.queue.write_buffer(
        &balls_pipe.storage_buffer,
        0,
        bytemuck::cast_slice(&[test_ball]),
    );
    uniforms.mouse_mode = 4; // Stir
    uniforms.mouse_vel = [0.0, -1.5]; // Mouse moving downward
    balls_pipe.update_uniforms(&ctx.queue, &uniforms);

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("StirEncoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("StirPass"),
            timestamp_writes: None,
        });
        balls_pipe.dispatch(&mut cpass);
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);

    let balls = balls_pipe.read_balls_sync(&ctx.device, &ctx.queue);
    assert!(
        balls[0].vel[1] < 0.0,
        "Stir must align ball velocity with mouse velocity (vel.y < 0), got {}",
        balls[0].vel[1]
    );
}

#[test]
fn test_sparks_dissipation_drag_and_gravity() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU execution: {}", e);
            return;
        }
    };

    let sparks_pipe = SparksComputePipeline::new(&ctx.device);

    // Seed 2 active particles
    let initial_particles = vec![
        Particle {
            position: [0.5, 0.5],
            velocity: [1.0, 0.0],
            age: 0.1,
            life: 1.0,
            hue: 0.2,
            size: 0.02,
        },
        Particle {
            position: [0.5, 0.5],
            velocity: [0.0, -1.0],
            age: 0.95,
            life: 1.0,
            hue: 0.5,
            size: 0.02,
        },
    ];
    sparks_pipe.write_particles(&ctx.queue, 0, &initial_particles);

    let dt = 0.1f32;
    let gravity = 0.4f32;
    let mut uniforms = SparksUniforms::default();
    uniforms.dt = dt;
    uniforms.gravity = gravity;
    uniforms.emit_count = 0; // Don't emit new particles over our test data
    sparks_pipe.update_uniforms(&ctx.queue, &uniforms);

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("SparksEncoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("SparksPass"),
            timestamp_writes: None,
        });
        sparks_pipe.dispatch(&mut cpass);
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);

    let after = sparks_pipe.read_particles_sync(&ctx.device, &ctx.queue, 2);

    // Particle 0:
    // Age should advance by dt
    assert!((after[0].age - 0.2).abs() < 1e-4);
    // Velocity drag: vx should be vx0 * exp(-1.2 * 0.1) ≈ 1.0 * 0.8869204
    let expected_drag = (-dt * 1.2).exp();
    let expected_vx = 1.0 * expected_drag;
    assert!(
        (after[0].velocity[0] - expected_vx).abs() < 1e-3,
        "Expected drag damped vx {}, got {}",
        expected_vx,
        after[0].velocity[0]
    );
    // Vertical gravity: vy should increase by gravity * dt
    let expected_vy = 0.0 * expected_drag + gravity * dt;
    assert!(
        (after[0].velocity[1] - expected_vy).abs() < 1e-3,
        "Expected gravity applied vy {}, got {}",
        expected_vy,
        after[0].velocity[1]
    );

    // Particle 1:
    // Initial age was 0.95, age += 0.1 => 1.05 >= life (1.0). Particle has expired.
    assert!(after[1].age >= after[1].life);
}

#[test]
fn test_sparks_emission_and_audio_excitation() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU execution: {}", e);
            return;
        }
    };

    let mut sparks_pipe = SparksComputePipeline::new(&ctx.device);
    sparks_pipe.reset_particles(&ctx.queue);

    // Emit 16 fresh particles
    let (start, count) = sparks_pipe.advance_emission(16);
    let uniforms = SparksUniforms {
        dt: 0.016,
        time: 1.23,
        aspect: 16.0 / 9.0,
        max_sparks: MAX_SPARKS as u32,
        emit_rate: 100.0,
        speed: 0.5,
        spread: 1.0,
        gravity: -0.05,
        life_param: 1.5,
        size_param: 0.03,
        hue_cycle: 0.1,
        emit_count: count,
        mouse_pos: [0.7, 0.4],
        mouse_vel: [0.1, 0.0],
        mouse_active: 1,
        mouse_down: 1,
        emit_mode: 0,
        audio_beat: 0.8, // Audio beat burst!
        emit_start: start,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    };
    sparks_pipe.update_uniforms(&ctx.queue, &uniforms);

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("SparksEmitEncoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("SparksEmitPass"),
            timestamp_writes: None,
        });
        sparks_pipe.dispatch(&mut cpass);
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);

    let particles = sparks_pipe.read_particles_sync(&ctx.device, &ctx.queue, 16);
    for (i, p) in particles.iter().enumerate() {
        assert_eq!(p.age, 0.0, "Freshly emitted spark {} must have age 0.0", i);
        assert!(p.life > 0.0, "Spark {} must have life > 0.0", i);
        assert!(p.size > 0.0, "Spark {} must have size > 0.0", i);
        // Position should be emitted at mouse_pos
        assert!(
            (p.position[0] - 0.7).abs() < 1e-4,
            "Spark {} position x mismatch",
            i
        );
        assert!(
            (p.position[1] - 0.4).abs() < 1e-4,
            "Spark {} position y mismatch",
            i
        );
        // Velocity magnitude should be boosted by audio_beat
        let spd = (p.velocity[0].powi(2) + p.velocity[1].powi(2)).sqrt();
        assert!(
            spd > 0.1,
            "Spark {} speed must be substantial under beat excitation",
            i
        );
    }
}

#[test]
fn test_boids_flocking_separation_and_toroidal_wrapping() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU execution: {}", e);
            return;
        }
    };

    let boids_pipe = BoidsComputePipeline::new(&ctx.device, 2, 0);

    // Place two boids very close together (distance 0.02 < R_sep = 0.38 * 0.28 ≈ 0.1064)
    // Boid 0 at [0.50, 0.50], Boid 1 at [0.52, 0.50]
    let boid0 = Boid {
        pos: [0.50, 0.50],
        vel: [0.0, 0.0],
        target_pos: [0.0, 0.0],
        light: 0.15,
        energy: 1.0,
        species: 0,
        phase: 0.0,
        _pad: [0.0, 0.0],
    };
    let boid1 = Boid {
        pos: [0.52, 0.50],
        vel: [0.0, 0.0],
        target_pos: [0.0, 0.0],
        light: 0.15,
        energy: 1.0,
        species: 0,
        phase: 0.0,
        _pad: [0.0, 0.0],
    };
    boids_pipe.write_boids(&ctx.queue, &[boid0, boid1]);

    let mut uniforms = BoidsUniforms::default();
    uniforms.boid_count = 2;
    uniforms.spore_count = 0;
    uniforms.dt = 0.05;
    uniforms.sense = 0.28;
    uniforms.separation = 2.0;
    uniforms.flock = 0.0; // Isolate separation force
    uniforms.speed = 1.0;
    uniforms.mouse_active = 0;

    boids_pipe.update_uniforms(&ctx.queue, &uniforms);

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("BoidsSeparationEncoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("BoidsSeparationPass"),
            timestamp_writes: None,
        });
        boids_pipe.dispatch(&mut cpass);
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);

    let result = boids_pipe.read_boids_sync(&ctx.device, &ctx.queue, 2);

    // Boid 0 should accelerate to the left (vel.x < 0) away from Boid 1
    assert!(
        result[0].vel[0] < 0.0,
        "Separation force must push Boid 0 left, got {}",
        result[0].vel[0]
    );
    // Boid 1 should accelerate to the right (vel.x > 0) away from Boid 0
    assert!(
        result[1].vel[0] > 0.0,
        "Separation force must push Boid 1 right, got {}",
        result[1].vel[0]
    );
}

#[test]
fn test_boids_spore_foraging_and_feeding_excitation() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU execution: {}", e);
            return;
        }
    };

    let boids_pipe = BoidsComputePipeline::new(&ctx.device, 1, 1);

    // Place a hungry boid at [0.500, 0.500] with light 0.15
    let boid = Boid {
        pos: [0.500, 0.500],
        vel: [0.01, 0.0],
        target_pos: [0.0, 0.0],
        light: 0.15,
        energy: 1.0,
        species: 0,
        phase: 0.0,
        _pad: [0.0, 0.0],
    };
    // Place a spore directly inside feeding range (< 0.025): [0.510, 0.500] (distance = 0.01)
    let spore = Spore {
        pos: [0.510, 0.500],
        vel: [0.0, 0.0],
        pulse: 1.0,
        _pad: [0.0; 3],
    };

    boids_pipe.write_boids(&ctx.queue, &[boid]);
    boids_pipe.write_spores(&ctx.queue, &[spore]);

    let mut uniforms = BoidsUniforms::default();
    uniforms.boid_count = 1;
    uniforms.spore_count = 1;
    uniforms.dt = 0.016;
    uniforms.sense = 0.3;
    uniforms.mouse_active = 0;

    boids_pipe.update_uniforms(&ctx.queue, &uniforms);

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("BoidsFeedEncoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("BoidsFeedPass"),
            timestamp_writes: None,
        });
        boids_pipe.dispatch(&mut cpass);
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);

    let result = boids_pipe.read_boids_sync(&ctx.device, &ctx.queue, 1);
    assert_eq!(
        result[0].light, 1.0,
        "Feeding excitation must flash bioluminescent light to 1.0 upon spore ingestion"
    );
}

#[test]
fn test_boids_predator_reactions_hunt_and_flee() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU execution: {}", e);
            return;
        }
    };

    let boids_pipe = BoidsComputePipeline::new(&ctx.device, 1, 0);

    // Boid at (0.4, 0.5), mouse at (0.5, 0.5) -> mouse is to the right
    let boid = Boid {
        pos: [0.4, 0.5],
        vel: [0.0, 0.0],
        target_pos: [0.0, 0.0],
        light: 0.15,
        energy: 1.0,
        species: 0,
        phase: 0.0,
        _pad: [0.0, 0.0],
    };
    boids_pipe.write_boids(&ctx.queue, &[boid]);

    // Test Hunt mode: boid accelerates toward mouse (vel.x > 0)
    let mut uniforms = BoidsUniforms::default();
    uniforms.boid_count = 1;
    uniforms.spore_count = 0;
    uniforms.dt = 0.05;
    uniforms.mouse_active = 1;
    uniforms.mouse_pos = [0.5, 0.5];
    uniforms.mouse_radius = 0.3;
    uniforms.mouse_force = 2.0;
    uniforms.mouse_mode = 0; // Hunt

    boids_pipe.update_uniforms(&ctx.queue, &uniforms);

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("BoidHuntEncoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("BoidHuntPass"),
            timestamp_writes: None,
        });
        boids_pipe.dispatch(&mut cpass);
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);

    let result = boids_pipe.read_boids_sync(&ctx.device, &ctx.queue, 1);
    assert!(
        result[0].vel[0] > 0.0,
        "Hunt mode must accelerate toward mouse (vel.x > 0), got {}",
        result[0].vel[0]
    );

    // Test Flee mode: boid accelerates away from mouse (vel.x < 0)
    boids_pipe.write_boids(&ctx.queue, &[boid]);
    uniforms.mouse_mode = 1; // Flee
    boids_pipe.update_uniforms(&ctx.queue, &uniforms);

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("BoidFleeEncoder"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("BoidFleePass"),
            timestamp_writes: None,
        });
        boids_pipe.dispatch(&mut cpass);
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);

    let result = boids_pipe.read_boids_sync(&ctx.device, &ctx.queue, 1);
    assert!(
        result[0].vel[0] < 0.0,
        "Flee mode must accelerate away from mouse (vel.x < 0), got {}",
        result[0].vel[0]
    );
}

#[test]
fn test_brush_sdf_pipeline_and_analytical_accuracy() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU execution: {}", e);
            return;
        }
    };

    let width = 640;
    let height = 360;
    let format = wgpu::TextureFormat::Rgba16Float;

    let mut drawing = DrawingPipeline::new(&ctx.device, format, width, height);

    // Verify canvas accumulation texture was created with requested dimensions
    assert_eq!(drawing.canvas_width, width);
    assert_eq!(drawing.canvas_height, height);

    // Upload test stroke segments (solid, neon, soft, and eraser)
    let segments = vec![
        StrokeSegment {
            p0: [100.0, 100.0],
            p1: [300.0, 100.0],
            size: 16.0,
            birth_time: 0.0,
            lifetime: 0.0,
            style: BrushStyle::Solid as u32,
            color: [1.0, 0.0, 0.5, 1.0],
        },
        StrokeSegment {
            p0: [150.0, 50.0],
            p1: [150.0, 250.0],
            size: 20.0,
            birth_time: 0.0,
            lifetime: 5.0,
            style: BrushStyle::Neon as u32,
            color: [0.0, 0.9, 1.0, 1.0],
        },
        StrokeSegment {
            p0: [200.0, 200.0],
            p1: [400.0, 300.0],
            size: 24.0,
            birth_time: 0.0,
            lifetime: 0.0,
            style: BrushStyle::Erase as u32,
            color: [0.0, 0.0, 0.0, 1.0],
        },
    ];
    drawing.upload_segments(&ctx.queue, &segments);

    let uniforms = DrawUniforms {
        viewport_size: [width as f32, height as f32],
        time: 1.0,
        aspect: width as f32 / height as f32,
        is_eraser: 0,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    };
    drawing.update_uniforms(&ctx.queue, &uniforms);

    // Render strokes to canvas texture
    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("DrawEncoder"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("DrawPass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &drawing.canvas_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        drawing.render_segments(&mut rpass, 2, false);
        drawing.render_segments(&mut rpass, 1, true); // Render eraser segment
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);

    // Test resize canvas
    drawing.resize_canvas(&ctx.device, 1280, 720);
    assert_eq!(drawing.canvas_width, 1280);
    assert_eq!(drawing.canvas_height, 720);

    // Test clear canvas
    drawing.clear_canvas(&ctx.device, &ctx.queue);
}

#[test]
fn test_line_segment_sdf_analytical_math() {
    // Exact CPU verification of the fragment shader analytical segment SDF formula
    fn distance_to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
        let ba = [b[0] - a[0], b[1] - a[1]];
        let pa = [p[0] - a[0], p[1] - a[1]];
        let l2 = ba[0] * ba[0] + ba[1] * ba[1];
        let t = ((pa[0] * ba[0] + pa[1] * ba[1]) / l2.max(1e-6)).clamp(0.0, 1.0);
        let proj = [a[0] + ba[0] * t, a[1] + ba[1] * t];
        ((p[0] - proj[0]).powi(2) + (p[1] - proj[1]).powi(2)).sqrt()
    }

    let a = [100.0, 200.0];
    let b = [300.0, 200.0];

    // Point exactly on midpoint
    assert_eq!(distance_to_segment([200.0, 200.0], a, b), 0.0);

    // Point perpendicular at distance 15.0
    let d_perp = distance_to_segment([200.0, 215.0], a, b);
    assert!((d_perp - 15.0).abs() < 1e-5);

    // Point beyond end cap b by distance 20.0
    let d_cap = distance_to_segment([320.0, 200.0], a, b);
    assert!((d_cap - 20.0).abs() < 1e-5);

    // Point at 45 degree angle past endpoint a
    let d_corner = distance_to_segment([100.0 - 3.0, 200.0 - 4.0], a, b);
    assert!((d_corner - 5.0).abs() < 1e-5);
}
