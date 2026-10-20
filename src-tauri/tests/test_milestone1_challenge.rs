// Empirical Challenge Test Suite for Milestone 1 (WGPU FX Engine Core)
// Tests edge cases: 0x0, 1x1, NPOT, 4K, rapid state transitions, idle zoom precision,
// uniform buffer stability, extreme parameter values, and GPU execution under stress.

use app_lib::graphics::{
    FxParams, FxStateMachine, PostProcessPipeline, PostProcessTargets, RenderTarget,
    WgpuContext,
};

#[test]
fn test_edge_case_zero_dimension_targets_all_passes() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    // Test creating RenderTarget with zero dimensions (0x0, 0x100, 100x0)
    let t_0x0 = RenderTarget::new(&ctx.device, 0, 0, wgpu::TextureFormat::Rgba16Float, Some("Zero_0x0"));
    let t_0x100 = RenderTarget::new(&ctx.device, 0, 100, wgpu::TextureFormat::Rgba16Float, Some("Zero_0x100"));
    let t_100x0 = RenderTarget::new(&ctx.device, 100, 0, wgpu::TextureFormat::Rgba16Float, Some("Zero_100x0"));

    // Check texture handles exist and textures are safely clamped to >= 1x1
    assert!(t_0x0.texture.width() >= 1);
    assert!(t_0x0.texture.height() >= 1);
    assert!(t_0x100.texture.width() >= 1);
    assert!(t_100x0.texture.height() >= 1);

    // Test PostProcessTargets with 0x0 and ALL passes enabled (Echo, Blur, Glow, Warp, Kaleido, Zoom)
    let mut targets_0 = PostProcessTargets::new(&ctx.device, 0, 0);

    let mut pipeline = PostProcessPipeline::new(&ctx.device, wgpu::TextureFormat::Rgba8UnormSrgb);
    let mut sm = FxStateMachine::new();
    sm.target.echo = 0.5;
    sm.target.blur = 0.8;
    sm.target.glow = 1.0;
    sm.target.warp = 0.05;
    sm.target.k_n = 6.0;
    sm.step(0.016);

    let uniforms = sm.build_uniforms(0, 0, 0.016);

    let output_target = RenderTarget::new(
        &ctx.device,
        1,
        1,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Some("Output_1x1"),
    );

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("ZeroDimAllPassesEncoder"),
    });

    pipeline.process(
        &ctx.device,
        &ctx.queue,
        &mut encoder,
        &mut targets_0,
        &uniforms,
        &output_target.view,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    );

    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);
}

#[test]
fn test_edge_case_1x1_and_npot_resolutions() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    let resolutions = [
        (1, 1),       // Minimum 1x1
        (2, 2),       // 2x2
        (3, 3),       // Odd small
        (13, 17),     // Prime NPOT
        (720, 480),   // NTSC standard
        (1365, 767),  // Common laptop odd resolution
        (1920, 1080), // Standard 1080p
    ];

    let mut pipeline = PostProcessPipeline::new(&ctx.device, wgpu::TextureFormat::Rgba8UnormSrgb);
    let mut sm = FxStateMachine::new();
    sm.target.echo = 0.5;
    sm.target.blur = 0.75;
    sm.target.glow = 1.0;
    sm.target.warp = 0.05;
    sm.target.k_n = 6.0;
    sm.target.rgb = 0.02;
    sm.step(0.016);

    for (w, h) in resolutions {
        let mut targets = PostProcessTargets::new(&ctx.device, w, h);
        let uniforms = sm.build_uniforms(w, h, 0.016);

        let output_target = RenderTarget::new(
            &ctx.device,
            w,
            h,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            Some("OutputTarget"),
        );

        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ResEncoder"),
        });

        pipeline.process(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &mut targets,
            &uniforms,
            &output_target.view,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );

        ctx.queue.submit(std::iter::once(encoder.finish()));
        ctx.device.poll(wgpu::Maintain::Wait);
    }
}

#[test]
fn test_edge_case_4k_resolution_execution() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    let w = 3840;
    let h = 2160;

    let mut targets = PostProcessTargets::new(&ctx.device, w, h);
    let mut pipeline = PostProcessPipeline::new(&ctx.device, wgpu::TextureFormat::Rgba8UnormSrgb);
    let mut sm = FxStateMachine::new();
    sm.target.echo = 0.6;
    sm.target.blur = 0.8;
    sm.target.glow = 1.2;
    sm.target.warp = 0.03;
    sm.target.zoom_speed = 0.2;
    sm.step(0.016);

    let uniforms = sm.build_uniforms(w, h, 0.016);

    let output_target = RenderTarget::new(
        &ctx.device,
        w,
        h,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Some("Output4K"),
    );

    let start = std::time::Instant::now();
    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("4KEncoder"),
    });

    pipeline.process(
        &ctx.device,
        &ctx.queue,
        &mut encoder,
        &mut targets,
        &uniforms,
        &output_target.view,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    );

    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);
    let duration = start.elapsed();
    println!("4K render and queue execution completed in: {:?}", duration);
    // 4K frame submission and GPU completion must not timeout or panic
    assert!(duration.as_secs() < 5);
}

#[test]
fn test_rapid_resize_thrashing() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    let mut targets = PostProcessTargets::new(&ctx.device, 1920, 1080);
    let mut pipeline = PostProcessPipeline::new(&ctx.device, wgpu::TextureFormat::Rgba8UnormSrgb);
    let sm = FxStateMachine::new();

    let sequence = [
        (1920, 1080),
        (3840, 2160),
        (1, 1),
        (640, 480),
        (2560, 1440),
        (0, 0),
        (1280, 720),
    ];

    for &(w, h) in &sequence {
        targets.resize(&ctx.device, w, h);
        let uniforms = sm.build_uniforms(w, h, 0.016);

        let out_w = w.max(1);
        let out_h = h.max(1);
        let output_target = RenderTarget::new(
            &ctx.device,
            out_w,
            out_h,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            Some("OutputTarget"),
        );

        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ResizeEncoder"),
        });

        pipeline.process(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &mut targets,
            &uniforms,
            &output_target.view,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );

        ctx.queue.submit(std::iter::once(encoder.finish()));
        ctx.device.poll(wgpu::Maintain::Wait);
    }
}

#[test]
fn test_idle_zoom_snapping_precision() {
    let mut sm = FxStateMachine::new();

    // 1. Zoom with velocity, then stop and test decay
    sm.target.zoom_speed = 0.5;
    for _ in 0..60 {
        sm.step(0.016);
    }
    assert!(sm.zoom_acc > 0.0);

    // Stop zoom
    sm.target.zoom_speed = 0.0;
    // Step for 240 frames (~4.0s). Deceleration takes ~0.5s, then exp(-2*t) decays residual < 0.001
    for _ in 0..240 {
        sm.step(0.016);
    }

    let nearest = sm.zoom_acc.round();
    let residual = (sm.zoom_acc - nearest).abs();
    assert!(
        residual < 0.001,
        "Zoom accumulator residual after 4s should be < 0.001, got {}",
        residual
    );

    // Uniform zoom_on should snap to 0.0 when idle
    let u = sm.build_uniforms(1920, 1080, 0.016);
    assert_eq!(
        u.zoom[3], 0.0,
        "zoom_on flag should be 0.0 when snapped to integer cycle"
    );

    // 2. Micro-velocity test: tiny velocity below idle threshold (0.002 < 0.01)
    sm.target.zoom_speed = 0.002;
    for _ in 0..30 {
        sm.step(0.016);
    }
    assert!(sm.cur.zoom_speed < 0.01);

    // 3. Negative zoom velocity and snapping
    sm.target.zoom_speed = -0.4;
    for _ in 0..60 {
        sm.step(0.016);
    }
    sm.target.zoom_speed = 0.0;
    for _ in 0..240 {
        sm.step(0.016);
    }
    let nearest_neg = sm.zoom_acc.round();
    let residual_neg = (sm.zoom_acc - nearest_neg).abs();
    assert!(
        residual_neg < 0.001,
        "Negative zoom accumulator residual should be < 0.001, got {}",
        residual_neg
    );
    let u_neg = sm.build_uniforms(1920, 1080, 0.016);
    assert_eq!(
        u_neg.zoom[3], 0.0,
        "Negative zoom zoom_on flag should be 0.0 when snapped"
    );
}

#[test]
fn test_uniform_buffer_stability_under_rapid_fluctuations() {
    let mut sm = FxStateMachine::new();

    // Rapid random/chaotic parameter updates
    for frame in 0..1000 {
        let t = frame as f32 * 0.016;
        let p = FxParams {
            speed: (t * 5.0).sin() * 2.0,
            zoom_speed: (t * 3.7).cos() * 1.5,
            zoom_depth: ((t * 1.3).sin() * 2.0).max(-3.0),
            zoom_twist: (t * 2.1).sin() * 180.0,
            k_n: ((frame % 12) as f32).max(0.0),
            k_spin: (t * 0.9).cos() * 50.0,
            k_rot: (t * 1.1).sin() * 360.0,
            k_zoom: 1.0 + (t * 2.5).sin() * 0.8,
            k_cx: (t * 4.0).sin() * 0.5,
            k_cy: (t * 4.3).cos() * 0.5,
            warp: (t * 6.0).sin().abs() * 0.2,
            warp_scale: 1.0 + (t * 1.7).cos().abs() * 5.0,
            warp_speed: (t * 3.0).sin() * 2.0,
            blur: (t * 2.2).sin().abs(),
            glow: (t * 1.9).cos().abs() * 2.0,
            sat: 1.0 + (t * 0.8).sin(),
            hue: t * 45.0,
            hue_spin: (t * 2.0).cos() * 90.0,
            exposure: (1.0 + (t * 1.5).sin() * 0.5).max(0.0),
            contrast: (1.0 + (t * 1.2).cos() * 0.5).max(0.0),
            echo: (t * 3.3).sin().abs().clamp(0.0, 0.99),
            echo_zoom: (t * 1.4).sin() * 0.1,
            echo_twist: (t * 0.7).cos() * 5.0,
            rgb: (t * 4.5).sin().abs() * 0.05,
            vig: (t * 0.5).cos().abs() * 0.8,
        };

        sm.set_target(p, Some(0.02));
        sm.step(0.016);

        let u = sm.build_uniforms(1920, 1080, 0.016);

        // Verify that all uniform buffer floats are finite (no NaN, no Inf)
        let bytes: &[f32] = bytemuck::cast_slice(bytemuck::bytes_of(&u));
        for (idx, &val) in bytes.iter().enumerate() {
            assert!(
                val.is_finite(),
                "Uniform float at index {} became non-finite: {} at frame {}",
                idx,
                val,
                frame
            );
        }
    }
}

#[test]
fn test_all_output_texture_formats() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    let formats = [
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Rgba16Float,
    ];

    let mut pipeline = PostProcessPipeline::new(&ctx.device, wgpu::TextureFormat::Rgba8UnormSrgb);
    let mut targets = PostProcessTargets::new(&ctx.device, 256, 256);
    let sm = FxStateMachine::new();
    let uniforms = sm.build_uniforms(256, 256, 0.016);

    for format in formats {
        let out_target = RenderTarget::new(&ctx.device, 256, 256, format, Some("FormatTarget"));

        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("FormatEncoder"),
        });

        pipeline.process(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &mut targets,
            &uniforms,
            &out_target.view,
            format,
        );

        ctx.queue.submit(std::iter::once(encoder.finish()));
        ctx.device.poll(wgpu::Maintain::Wait);
    }
}

#[test]
fn test_extreme_parameter_gpu_execution() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    let mut pipeline = PostProcessPipeline::new(&ctx.device, wgpu::TextureFormat::Rgba8UnormSrgb);
    let mut targets = PostProcessTargets::new(&ctx.device, 512, 512);

    let extreme_cases = [
        // Echo zoom = 0 / negative
        FxParams { echo: 0.9, echo_zoom: -1.0, ..FxParams::default() },
        FxParams { echo: 0.9, echo_zoom: -5.0, ..FxParams::default() },
        // Kaleidoscope boundary segments
        FxParams { k_n: 0.0, ..FxParams::default() },
        FxParams { k_n: 0.49, ..FxParams::default() },
        FxParams { k_n: 0.5, ..FxParams::default() },
        FxParams { k_n: 1.0, ..FxParams::default() },
        FxParams { k_n: 24.0, ..FxParams::default() },
        FxParams { k_n: 100.0, ..FxParams::default() },
        // Zoom depth boundaries
        FxParams { zoom_speed: 1.0, zoom_depth: -5.0, ..FxParams::default() },
        FxParams { zoom_speed: 1.0, zoom_depth: 5.0, ..FxParams::default() },
        // Extreme warp and scale
        FxParams { warp: 1.0, warp_scale: 100.0, warp_speed: 10.0, ..FxParams::default() },
        // Extreme blur and glow
        FxParams { blur: 1.0, glow: 5.0, ..FxParams::default() },
        // Extreme color grading
        FxParams { exposure: 10.0, contrast: 5.0, sat: 5.0, rgb: 0.5, vig: 1.0, ..FxParams::default() },
    ];

    let output_target = RenderTarget::new(
        &ctx.device,
        512,
        512,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Some("ExtremeOutputTarget"),
    );

    for (i, params) in extreme_cases.into_iter().enumerate() {
        let mut sm = FxStateMachine::new();
        sm.cur = params;
        sm.step(0.016);
        let uniforms = sm.build_uniforms(512, 512, 0.016);

        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some(&format!("ExtremeEncoder_{}", i)),
        });

        pipeline.process(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &mut targets,
            &uniforms,
            &output_target.view,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );

        ctx.queue.submit(std::iter::once(encoder.finish()));
        ctx.device.poll(wgpu::Maintain::Wait);
    }
}

#[test]
fn test_temporal_delta_time_extremes() {
    let mut sm = FxStateMachine::new();
    sm.target.speed = 2.0;
    sm.target.zoom_speed = 0.5;

    // Zero dt
    sm.step(0.0);
    let u_zero = sm.build_uniforms(1920, 1080, 0.0);
    assert!(u_zero.aspect_time[2] == 0.0);

    // Negative dt (backward clock step)
    sm.step(-0.016);
    let u_neg = sm.build_uniforms(1920, 1080, -0.016);
    let bytes: &[f32] = bytemuck::cast_slice(bytemuck::bytes_of(&u_neg));
    for &val in bytes {
        assert!(val.is_finite(), "Uniform float must remain finite on negative dt");
    }

    // Huge dt (window unfocused for 100 seconds)
    sm.step(100.0);
    let u_huge = sm.build_uniforms(1920, 1080, 100.0);
    let bytes_huge: &[f32] = bytemuck::cast_slice(bytemuck::bytes_of(&u_huge));
    for &val in bytes_huge {
        assert!(val.is_finite(), "Uniform float must remain finite on large dt");
    }
    // Echo amount must not NaN or blow up
    assert!(u_huge.feedback[0] >= 0.0 && u_huge.feedback[0] <= 1.0);
}
