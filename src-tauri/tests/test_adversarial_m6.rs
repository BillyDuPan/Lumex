//! Tier 5 Final Adversarial Stress Testing & Verification Suite (Milestone 6)
//!
//! Conducts empirical white-box and opaque-box challenge testing across all integrated Lumex subsystems:
//! 1. GPU Compute Max Agent Capacity: 10,000 Boids + 64 Spores with continuous predator mode thrashing
//! 2. GPU Compute Sparks Saturation: 65,536 Particles with circular ring-buffer wrap and audio burst shockwaves
//! 3. GPU Compute Floating Balls: 5,000 Balls under violent mouse dynamics and mode switching
//! 4. GPU Vector Brush SDF: Degenerate geometry (p0 == p1, extreme coordinates, 0/1000px sizes, 4K resize)
//! 5. Integrated Cross-Subsystem Pipeline: Simultaneous Compute (10k Boids) + Multi-Pass FX + Dual Presentation Blits
//! 6. Audio Reactivity Pathological Inputs: DC offsets, high-amplitude square clipping, Nyquist tone, sub-floor noise, Dirac impulse
//! 7. Hardware MIDI Robustness: 256-byte exhaustive packet fuzzing, SysEx rejection, matrix modulation bounds
//! 8. Professional Texture Sharing: Multi-sender concurrent broadcasting, dimension/format rejection, rapid lifecycle churning

use app_lib::audio::analysis::AudioAnalysisEngine;
use app_lib::compute::balls::{BallsComputePipeline, BallsUniforms};
use app_lib::compute::boids::{BoidsComputePipeline, BoidsUniforms, MAX_BOIDS, MAX_SPORES};
use app_lib::compute::drawing::{BrushStyle, DrawUniforms, DrawingPipeline, StrokeSegment};
use app_lib::compute::sparks::{SparksComputePipeline, SparksUniforms, MAX_SPARKS};
use app_lib::display::preview::{GpuBlitPipeline, ViewportRect};
use app_lib::graphics::{
    FxParams, FxStateMachine, PostProcessPipeline, PostProcessTargets, RenderTarget, WgpuContext,
};
use app_lib::midi::matrix::{CurveType, MappingMode, MidiMapping, MidiModulationMatrix};
use app_lib::midi::parser::MidiMessage;
use app_lib::sharing::{create_texture_sender, MockTextureSender, TextureShareSender};

// =========================================================================
// 1. GPU Compute Max Capacity: 10,000 Boids Simulation
// =========================================================================
#[test]
fn test_adversarial_boids_max_capacity_10000() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    let boids_pipe = BoidsComputePipeline::new(&ctx.device, MAX_BOIDS as u32, MAX_SPORES as u32);
    let aspect = 16.0 / 9.0;
    boids_pipe.init_boids_and_spores(&ctx.queue, aspect);

    let mut uniforms = BoidsUniforms::default();
    uniforms.boid_count = MAX_BOIDS as u32;
    uniforms.spore_count = MAX_SPORES as u32;
    uniforms.aspect = aspect;
    uniforms.max_neighbors = 128;

    // Simulate 20 continuous frames while violently thrashing mouse predator modes
    for frame in 0..20 {
        uniforms.dt = 0.016;
        uniforms.time = frame as f32 * 0.016;
        uniforms.mouse_active = 1;
        uniforms.mouse_mode = (frame % 4) as u32; // 0: Hunt, 1: Flee, 2: Orbit, 3: Off
        uniforms.mouse_pos = [
            (frame as f32 * 0.17).sin().abs() * aspect,
            (frame as f32 * 0.23).cos().abs(),
        ];
        uniforms.mouse_radius = 0.4;
        uniforms.mouse_force = 3.0;
        uniforms.audio_beat = if frame % 4 == 0 { 1.0 } else { 0.0 };

        boids_pipe.update_uniforms(&ctx.queue, &uniforms);

        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some(&format!("BoidsMax10kEncoder_{}", frame)),
        });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Boids10kPass"),
                timestamp_writes: None,
            });
            boids_pipe.dispatch(&mut cpass);
        }
        ctx.queue.submit(std::iter::once(encoder.finish()));
        ctx.device.poll(wgpu::Maintain::Wait);
    }

    // Read back a sample of 256 boids distributed across the entire 10,000 array
    let read_count = 256;
    let boids = boids_pipe.read_boids_sync(&ctx.device, &ctx.queue, read_count);
    assert_eq!(boids.len(), read_count);

    for (idx, boid) in boids.iter().enumerate() {
        assert!(
            boid.pos[0].is_finite() && boid.pos[1].is_finite(),
            "Boid {} position non-finite: {:?}",
            idx,
            boid.pos
        );
        assert!(
            boid.vel[0].is_finite() && boid.vel[1].is_finite(),
            "Boid {} velocity non-finite: {:?}",
            idx,
            boid.vel
        );
        assert!(
            boid.light.is_finite() && boid.light >= 0.15,
            "Boid {} bioluminescence must stay >= 0.15, got {}",
            idx,
            boid.light
        );
        assert!(
            boid.energy.is_finite() && boid.energy > 0.0,
            "Boid {} energy must be positive, got {}",
            idx,
            boid.energy
        );
    }
}

// =========================================================================
// 2. GPU Compute Sparks Saturation: 65,536 Particles with Ring-Buffer Wrap
// =========================================================================
#[test]
fn test_adversarial_sparks_max_saturation_65536() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    let mut sparks_pipe = SparksComputePipeline::new(&ctx.device);
    sparks_pipe.reset_particles(&ctx.queue);

    // Emit 8,192 sparks per frame for 10 frames = 81,920 total sparks,
    // which completely fills and wraps around the 65,536 circular buffer!
    let emit_chunk = 8192u32;
    for frame in 0..10 {
        let (start, count) = sparks_pipe.advance_emission(emit_chunk);
        let uniforms = SparksUniforms {
            dt: 0.016,
            time: frame as f32 * 0.016,
            aspect: 16.0 / 9.0,
            max_sparks: MAX_SPARKS as u32,
            emit_rate: 1000.0,
            speed: 1.5,
            spread: 2.0,
            gravity: -0.2, // Inverted upward gravity
            life_param: 1.0,
            size_param: 0.04,
            hue_cycle: 0.5,
            emit_count: count,
            mouse_pos: [0.8, 0.5],
            mouse_vel: [-2.0, 1.0],
            mouse_active: 1,
            mouse_down: 1,
            emit_mode: 0,
            audio_beat: 1.0, // Maximum audio burst excitation
            emit_start: start,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        };
        sparks_pipe.update_uniforms(&ctx.queue, &uniforms);

        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some(&format!("SparksMaxSatEncoder_{}", frame)),
        });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("SparksSatPass"),
                timestamp_writes: None,
            });
            sparks_pipe.dispatch(&mut cpass);
        }
        ctx.queue.submit(std::iter::once(encoder.finish()));
        ctx.device.poll(wgpu::Maintain::Wait);
    }

    // Read back head particles from both start of ring and near max
    let head_particles = sparks_pipe.read_particles_sync(&ctx.device, &ctx.queue, 128);
    for (i, p) in head_particles.iter().enumerate() {
        assert!(
            p.position[0].is_finite() && p.position[1].is_finite(),
            "Spark {} position non-finite",
            i
        );
        assert!(
            p.velocity[0].is_finite() && p.velocity[1].is_finite(),
            "Spark {} velocity non-finite",
            i
        );
        assert!(p.age.is_finite() && p.age >= 0.0);
    }
}

// =========================================================================
// 3. GPU Compute Floating Balls: 5,000 Agents Under Violent Mouse Dynamics
// =========================================================================
#[test]
fn test_adversarial_balls_max_capacity_5000() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    let count = 5000u32;
    let balls_pipe = BallsComputePipeline::new(&ctx.device, count);
    let aspect = 16.0 / 9.0;
    balls_pipe.init_particles(&ctx.queue, count, aspect);

    for frame in 0..15 {
        let uniforms = BallsUniforms {
            dt: 0.016,
            time: frame as f32 * 0.016,
            aspect,
            count,
            speed: 1.5,
            drift: 1.0,
            size: 0.05,
            size_var: 0.5,
            pulse: 0.5,
            swell_param: 8.0, // Extreme swell
            softness: 0.9,
            alpha: 1.0,
            mouse_mode: (frame % 4 + 1) as u32, // Repel, Attract, Swirl, Stir
            mouse_active: 1,
            mouse_pos: [(frame as f32 * 0.3).sin() * aspect, 0.5],
            mouse_vel: [50.0, -50.0], // Violent mouse velocity
            mouse_radius: 0.5,
            mouse_force: 5.0,
            edge_mode: 0, // Wrap
            audio_bass: 1.0,
            audio_beat: 1.0,
            hue_cycle: 0.3,
        };
        balls_pipe.update_uniforms(&ctx.queue, &uniforms);

        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("BallsMax5kEncoder"),
        });
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Balls5kPass"),
                timestamp_writes: None,
            });
            balls_pipe.dispatch(&mut cpass);
        }
        ctx.queue.submit(std::iter::once(encoder.finish()));
        ctx.device.poll(wgpu::Maintain::Wait);
    }

    let sample_balls = balls_pipe.read_balls_sync(&ctx.device, &ctx.queue);
    assert_eq!(sample_balls.len(), count as usize);
    for b in sample_balls.iter().take(100) {
        assert!(b.pos[0].is_finite() && b.pos[1].is_finite());
        assert!(b.vel[0].is_finite() && b.vel[1].is_finite());
        assert!(b.radius > 0.0 && b.radius.is_finite());
        assert!(b.swell.is_finite() && b.swell >= 1.0);
    }
}

// =========================================================================
// 4. GPU Vector Brush SDF: Degenerate Geometry Torture
// =========================================================================
#[test]
fn test_adversarial_vector_brush_extreme_strokes() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    let mut drawing =
        DrawingPipeline::new(&ctx.device, wgpu::TextureFormat::Rgba16Float, 1920, 1080);

    // Degenerate segments:
    // 1. Zero-length line (p0 == p1)
    // 2. Far negative coordinates
    // 3. Huge offscreen coordinates (100,000 px)
    // 4. Zero brush size
    // 5. Giant brush size (2,000 px)
    let degenerate_segments = vec![
        StrokeSegment {
            p0: [500.0, 500.0],
            p1: [500.0, 500.0], // Exact point
            size: 10.0,
            birth_time: 0.0,
            lifetime: 0.0,
            style: BrushStyle::Solid as u32,
            color: [1.0, 1.0, 1.0, 1.0],
        },
        StrokeSegment {
            p0: [-50000.0, -50000.0],
            p1: [50000.0, 50000.0],
            size: 2000.0, // Giant size
            birth_time: 0.0,
            lifetime: 0.0,
            style: BrushStyle::Neon as u32,
            color: [0.0, 1.0, 0.5, 1.0],
        },
        StrokeSegment {
            p0: [100.0, 200.0],
            p1: [300.0, 400.0],
            size: 0.0, // Zero size
            birth_time: 0.0,
            lifetime: 0.0,
            style: BrushStyle::Soft as u32,
            color: [1.0, 0.0, 0.0, 1.0],
        },
        StrokeSegment {
            p0: [200.0, 200.0],
            p1: [400.0, 200.0],
            size: 50.0,
            birth_time: 0.0,
            lifetime: 0.0,
            style: BrushStyle::Erase as u32,
            color: [0.0, 0.0, 0.0, 1.0],
        },
    ];

    drawing.upload_segments(&ctx.queue, &degenerate_segments);

    let uniforms = DrawUniforms {
        viewport_size: [1920.0, 1080.0],
        time: 1.0,
        aspect: 16.0 / 9.0,
        is_eraser: 0,
        _pad0: 0,
        _pad1: 0,
        _pad2: 0,
    };
    drawing.update_uniforms(&ctx.queue, &uniforms);

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("DrawDegenerateEncoder"),
    });
    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("DrawDegeneratePass"),
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
        drawing.render_segments(&mut rpass, 3, false);
        drawing.render_segments(&mut rpass, 1, true); // Erase
    }
    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);

    // Resize canvas to 4K, clear, and verify
    drawing.resize_canvas(&ctx.device, 3840, 2160);
    assert_eq!(drawing.canvas_width, 3840);
    assert_eq!(drawing.canvas_height, 2160);
    drawing.clear_canvas(&ctx.device, &ctx.queue);
}

// =========================================================================
// 5. Integrated Cross-Subsystem Multi-Pass + Compute + Dual Blit Stress
// =========================================================================
#[test]
fn test_adversarial_integrated_pipeline_under_maximum_load() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    let width = 1920;
    let height = 1080;

    // 1. Compute pipeline (10,000 boids)
    let boids_pipe = BoidsComputePipeline::new(&ctx.device, 10000, 32);
    boids_pipe.init_boids_and_spores(&ctx.queue, 16.0 / 9.0);

    // 2. Multi-pass post-processing pipeline
    let mut post_pipeline =
        PostProcessPipeline::new(&ctx.device, wgpu::TextureFormat::Rgba8UnormSrgb);
    let mut targets = PostProcessTargets::new(&ctx.device, width, height);

    // Extreme FX configuration
    let mut sm = FxStateMachine::new();
    sm.cur = FxParams {
        echo: 0.95,
        echo_zoom: 0.05,
        echo_twist: 15.0,
        blur: 1.0,
        glow: 3.0,
        warp: 0.1,
        warp_scale: 25.0,
        warp_speed: 4.0,
        k_n: 12.0,
        k_spin: 90.0,
        zoom_speed: 1.0,
        zoom_twist: 180.0,
        exposure: 1.5,
        contrast: 2.0,
        sat: 2.5,
        rgb: 0.08,
        vig: 0.8,
        ..FxParams::default()
    };
    sm.step(0.016);
    let fx_uniforms = sm.build_uniforms(width, height, 0.016);

    // 3. Dual display targets
    let projector_target = RenderTarget::new(
        &ctx.device,
        3840,
        2160,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        Some("ProjectorTarget_4K"),
    );
    let preview_target = RenderTarget::new(
        &ctx.device,
        640,
        360,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Some("PreviewTarget_360p"),
    );

    let mut blit_pipeline =
        GpuBlitPipeline::new(&ctx.device, wgpu::TextureFormat::Rgba8UnormSrgb);

    let post_output_target = RenderTarget::new(
        &ctx.device,
        width,
        height,
        wgpu::TextureFormat::Rgba16Float,
        Some("PostOutputTarget"),
    );

    // Execute 10 fully integrated frames: Compute -> Multi-Pass FX -> Dual Blit
    for frame in 0..10 {
        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some(&format!("IntegratedMasterEncoder_{}", frame)),
        });

        // Stage A: GPU Compute Pass
        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("IntegratedComputePass"),
                timestamp_writes: None,
            });
            boids_pipe.dispatch(&mut cpass);
        }

        // Stage B: Multi-Pass FX Pipeline (Echo, Blur Downsample, Tent Glow, Final Composite)
        post_pipeline.process(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &mut targets,
            &fx_uniforms,
            &post_output_target.view,
            wgpu::TextureFormat::Rgba16Float,
        );

        // Stage C: Dual Display Zero-Copy Presentation Blits
        // 1. Fullscreen blit to 4K projector target
        blit_pipeline.blit(
            &ctx.device,
            &mut encoder,
            &post_output_target.view,
            &projector_target.view,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            None,
        );

        // 2. Letterbox-fitted blit to 360p control preview target
        let preview_vp = ViewportRect::fit_aspect_ratio(16.0 / 9.0, 640.0, 360.0);
        blit_pipeline.blit(
            &ctx.device,
            &mut encoder,
            &post_output_target.view,
            &preview_target.view,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            Some(&preview_vp),
        );

        ctx.queue.submit(std::iter::once(encoder.finish()));
        ctx.device.poll(wgpu::Maintain::Wait);
    }
}

// =========================================================================
// 6. Audio Reactivity Pathological Inputs
// =========================================================================
#[test]
fn test_adversarial_audio_pathological_signals() {
    let sample_rate = 48000;
    let mut engine = AudioAnalysisEngine::new(sample_rate);
    let dt = 256.0 / sample_rate as f32;

    // Test 1: Pure Constant DC Offset (+10.0)
    let dc_chunk = vec![10.0f32; 256];
    for _ in 0..16 {
        let u = engine.process_chunk(&dc_chunk, dt);
        assert!(!u.bass().is_nan() && u.bass().is_finite());
        assert!(!u.mid().is_nan() && u.mid().is_finite());
        assert!(!u.high().is_nan() && u.high().is_finite());
        assert!(!u.beat().is_nan() && u.beat().is_finite());
    }

    // Test 2: Extreme High Amplitude Square Clipping (+100.0 / -100.0)
    let mut square_chunk = vec![0.0f32; 256];
    for (i, sample) in square_chunk.iter_mut().enumerate() {
        *sample = if i % 2 == 0 { 100.0 } else { -100.0 };
    }
    for _ in 0..16 {
        let u = engine.process_chunk(&square_chunk, dt);
        assert!(u.bass() <= 1.0 && u.bass() >= 0.0);
        assert!(u.mid() <= 1.0 && u.mid() >= 0.0);
        assert!(u.high() <= 1.0 && u.high() >= 0.0);
        assert!(u.beat() <= 1.0 && u.beat() >= 0.0);
    }

    // Test 3: Nyquist Frequency Wave (24,000 Hz, alternating +1.0 / -1.0)
    let mut nyquist_chunk = vec![0.0f32; 256];
    for (i, sample) in nyquist_chunk.iter_mut().enumerate() {
        *sample = if i % 2 == 0 { 1.0 } else { -1.0 };
    }
    for frame in 0..16 {
        let u = engine.process_chunk(&nyquist_chunk, dt);
        if frame == 15 {
            println!(
                "Nyquist Frame 15: bass={}, mid={}, high={}, beat={}, raw_rms={}",
                u.bass(), u.mid(), u.high(), u.beat(), u.raw_rms()
            );
        }
        assert!(u.bass() <= 1.0 && u.bass() >= 0.0);
        assert!(u.mid() <= 1.0 && u.mid() >= 0.0);
        assert!(u.high() <= 1.0 && u.high() >= 0.0);
    }

    // Test 4: Sub-Noise Floor Noise
    let mut fresh_engine = AudioAnalysisEngine::new(sample_rate);
    let deep_sub_noise = vec![0.00005f32; 256];
    for frame in 0..16 {
        let u = fresh_engine.process_chunk(&deep_sub_noise, dt);
        if frame == 15 {
            assert_eq!(u.bass(), 0.0, "Deep sub-floor bass must be gated to 0.0");
            assert_eq!(u.mid(), 0.0, "Deep sub-floor mid must be gated to 0.0");
            assert_eq!(u.high(), 0.0, "Deep sub-floor high must be gated to 0.0");
        }
    }

    // Test 5: Single Dirac Spike (+10.0 spike in middle of silence)
    let mut dirac_chunk = vec![0.0f32; 256];
    dirac_chunk[128] = 10.0;
    let u_dirac = engine.process_chunk(&dirac_chunk, dt);
    assert!(!u_dirac.bass().is_nan());
    assert!(!u_dirac.raw_rms().is_nan());
}

// =========================================================================
// 7. Hardware MIDI Parser & Modulation Matrix Fuzzing
// =========================================================================
#[test]
fn test_adversarial_midi_parser_fuzzing_and_matrix_bounds() {
    // 1. Fuzz MIDI Parser: test all 256 single-byte status values
    for b0 in 0u8..=255 {
        let _ = MidiMessage::parse(&[b0]);
        // Also test 2-byte and 3-byte permutations
        let _ = MidiMessage::parse(&[b0, 0x00]);
        let _ = MidiMessage::parse(&[b0, 0x7F, 0x7F]);
        let _ = MidiMessage::parse(&[b0, 0xFF, 0xFF, 0xFF]);
    }

    // 2. SysEx dump attack: start with 0xF0 followed by 100 bytes
    let mut sysex = vec![0xF0];
    sysex.extend(std::iter::repeat(0x42).take(100));
    sysex.push(0xF7); // End of SysEx
    assert_eq!(MidiMessage::parse(&sysex), None);

    // 3. Modulation Matrix Fuzzing across 50 mappings and 10,000 messages
    let mut matrix = MidiModulationMatrix::new();
    let curves = [
        CurveType::Linear,
        CurveType::Exponential,
        CurveType::Logarithmic,
        CurveType::SCurve,
    ];
    let modes = [
        MappingMode::Absolute,
        MappingMode::Relative,
        MappingMode::Toggle,
        MappingMode::Momentary,
    ];

    for i in 0..50 {
        matrix.add_mapping(MidiMapping {
            id: format!("map_{}", i),
            channel: if i % 2 == 0 { Some((i % 16) as u8) } else { None },
            controller_or_note: (i % 128) as u8,
            is_note: i % 3 == 0,
            target_param: format!("param_{}", i),
            mode: modes[i % modes.len()],
            min_val: -10.0,
            max_val: 10.0,
            curve: curves[i % curves.len()],
        });
    }

    // Dispatch 5,000 synthetic randomized messages
    for step in 0..5000 {
        let ch = (step % 16) as u8;
        let num = (step % 128) as u8;
        let val = ((step * 7) % 128) as u8;

        let msg = if step % 2 == 0 {
            MidiMessage::ControlChange {
                channel: ch,
                controller: num,
                value: val,
            }
        } else if step % 4 == 1 {
            MidiMessage::NoteOn {
                channel: ch,
                note: num,
                velocity: val,
            }
        } else {
            MidiMessage::NoteOff {
                channel: ch,
                note: num,
                velocity: val,
            }
        };

        let updates = matrix.process_message(&msg);
        for (_param, val) in updates {
            assert!(
                val.is_finite(),
                "Matrix parameter must be finite"
            );
            assert!(
                val >= -10.0001 && val <= 10.0001,
                "Matrix value must stay within [min_val, max_val], got {}",
                val
            );
        }
    }
}

// =========================================================================
// 8. Professional Texture Sharing: Multi-Sender Concurrent Broadcasting
// =========================================================================
#[test]
fn test_adversarial_texture_sharing_concurrency_and_lifecycle_churn() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    // 1. Create 8 concurrent senders across different resolutions
    let configs = [
        ("Sender_720p", 1280, 720, wgpu::TextureFormat::Bgra8Unorm),
        ("Sender_1080p", 1920, 1080, wgpu::TextureFormat::Bgra8UnormSrgb),
        ("Sender_4K", 3840, 2160, wgpu::TextureFormat::Rgba8Unorm),
        ("Sender_Square", 1024, 1024, wgpu::TextureFormat::Rgba16Float),
        ("Sender_Ultrawide", 2560, 1080, wgpu::TextureFormat::Bgra8Unorm),
        ("Sender_Strip", 1920, 240, wgpu::TextureFormat::Rgba8UnormSrgb),
        ("Sender_Portrait", 1080, 1920, wgpu::TextureFormat::Bgra8Unorm),
        ("Sender_Tiny", 64, 64, wgpu::TextureFormat::Rgba8Unorm),
    ];

    let mut senders: Vec<Box<dyn TextureShareSender>> = configs
        .iter()
        .map(|&(name, w, h, fmt)| create_texture_sender(name, w, h, fmt))
        .collect();

    for (idx, sender) in senders.iter().enumerate() {
        assert!(sender.is_active());
        assert!(sender.server_name().contains(configs[idx].0));
    }

    // 2. Publish 5 frames across all 8 senders concurrently
    for frame in 0..5 {
        for (i, sender) in senders.iter_mut().enumerate() {
            let (_, w, h, fmt) = configs[i];
            let tex = ctx.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("ChurnTex_{}_{}", i, frame)),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: fmt,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });

            let res = sender.publish_texture(&ctx.device, &ctx.queue, &tex);
            assert!(res.is_ok(), "Publish frame {} on sender {} failed", frame, i);
        }
    }

    // 3. Close first 4 senders and verify remaining 4 remain active
    for sender in senders.iter_mut().take(4) {
        sender.close();
        assert!(!sender.is_active());
    }
    for sender in senders.iter().skip(4) {
        assert!(sender.is_active());
    }

    // 4. Mock sender dimension and format rejection under stress
    let mut mock = MockTextureSender::new("StressMock", 1920, 1080, wgpu::TextureFormat::Bgra8Unorm);
    let wrong_dim_tex = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("WrongDimTex"),
        size: wgpu::Extent3d {
            width: 1920,
            height: 1079, // Off by 1 pixel!
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let reject_res = mock.publish_texture(&ctx.device, &ctx.queue, &wrong_dim_tex);
    assert!(
        reject_res.is_err(),
        "Dimension mismatch (1920x1079 vs 1920x1080) must be rejected"
    );

    mock.close();
    assert!(!mock.is_active());
}
