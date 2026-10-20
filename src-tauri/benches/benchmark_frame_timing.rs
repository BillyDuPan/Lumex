//! Automated 1000-Frame Benchmark Binary for Lumex
//!
//! Measures microsecond-accurate frame render times over 1000 frames under
//! full multi-pass GPU post-processing load (Echo, Blur Downsampling, Tent Glow Composite,
//! Final FX Composite) at 1080p (1920x1080) headless offscreen target.
//!
//! Computes mean (<16.66ms for 60+ FPS), min, max, p50, p90, p95, and p99 frame timing.

use std::time::{Duration, Instant};

use app_lib::graphics::{
    FxStateMachine, PostProcessPipeline, PostProcessTargets, RenderTarget, WgpuContext,
};

const WARMUP_FRAMES: usize = 50;
const BENCHMARK_FRAMES: usize = 1000;
const TARGET_WIDTH: u32 = 1920;
const TARGET_HEIGHT: u32 = 1080;
const MAX_ALLOWED_MEAN_MS: f64 = 16.66; // Sustained 60+ FPS threshold

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("============================================================");
    println!(" Lumex High-Performance Automated Benchmark (1000 Frames)   ");
    println!(
        " Resolution: {}x{} (1080p) | Full Multi-Pass Post-Processing",
        TARGET_WIDTH, TARGET_HEIGHT
    );
    println!("============================================================");

    // 1. Initialize WGPU Headless Context
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to initialize WGPU headless context: {}", e);
            std::process::exit(1);
        }
    };

    let adapter_info = ctx.adapter.get_info();
    println!("GPU Adapter:     {}", adapter_info.name);
    println!("Graphics Backend: {:?}", adapter_info.backend);
    println!("Device Type:      {:?}", adapter_info.device_type);

    // 2. Initialize Offscreen Targets & Multi-Pass Pipeline at 1080p
    let output_format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut targets = PostProcessTargets::new(&ctx.device, TARGET_WIDTH, TARGET_HEIGHT);
    let mut pipeline = PostProcessPipeline::new(&ctx.device, output_format);
    let output_target = RenderTarget::new(
        &ctx.device,
        TARGET_WIDTH,
        TARGET_HEIGHT,
        output_format,
        Some("BenchmarkOutputTarget"),
    );

    // 3. Configure State Machine with Full Pipeline Load (All 4 Passes Active)
    let mut sm = FxStateMachine::new();
    sm.target.echo = 0.65;        // Pass 1: Feedback Echo ping-pong accumulation
    sm.target.blur = 0.50;        // Pass 2: 13-tap blur pyramid (4 mip levels)
    sm.target.glow = 0.80;        // Pass 3: 4-tap tent upsampling bloom composite
    sm.target.warp = 0.05;        // Pass 4: Sinusoidal liquid coordinate warp
    sm.target.k_n = 6.0;          // Pass 4: 6-segment kaleidoscope folding
    sm.target.zoom_speed = 0.40;  // Pass 4: Infinite logarithmic zoom with octave crossfade
    sm.target.zoom_twist = 0.20;  // Pass 4: Zoom rotational twist
    sm.target.contrast = 1.15;    // Pass 4: Rec.709 contrast S-curve
    sm.target.sat = 1.10;         // Pass 4: Rec.709 saturation scaling
    sm.target.rgb = 0.005;        // Pass 4: Radial chromatic aberration

    let dt = 0.016; // 16ms simulation step

    // 4. Warm-up Phase (50 Frames)
    println!("\nRunning {} warm-up frames...", WARMUP_FRAMES);
    for _ in 0..WARMUP_FRAMES {
        sm.step(dt);
        let uniforms = sm.build_uniforms(TARGET_WIDTH, TARGET_HEIGHT, dt);

        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("WarmupEncoder"),
        });

        // Clear scene source target to simulate active visual scene rendering
        {
            let _scene_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("WarmupScenePass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.src_target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.05,
                            b: 0.2,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // Execute 4-pass post-processing pipeline
        pipeline.process(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &mut targets,
            &uniforms,
            &output_target.view,
            output_format,
        );

        ctx.queue.submit(std::iter::once(encoder.finish()));
        ctx.device.poll(wgpu::Maintain::Wait);
    }
    println!("Warm-up complete.");

    // 5. Benchmark Execution Phase (1000 Frames)
    println!(
        "\nExecuting {} benchmark frames under full pipeline load...",
        BENCHMARK_FRAMES
    );
    let mut frame_durations: Vec<Duration> = Vec::with_capacity(BENCHMARK_FRAMES);

    let benchmark_start = Instant::now();

    for frame in 0..BENCHMARK_FRAMES {
        let frame_start = Instant::now();

        sm.step(dt);
        let uniforms = sm.build_uniforms(TARGET_WIDTH, TARGET_HEIGHT, dt);

        let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("BenchmarkFrameEncoder"),
        });

        // Render base scene pass
        {
            let _scene_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("BenchmarkScenePass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.src_target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: ((frame % 100) as f64) / 100.0,
                            g: 0.2,
                            b: 0.6,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // Execute full multi-pass pipeline:
        // Pass 1: Echo -> Pass 2: Blur Downsampling -> Pass 3: Tent Glow -> Pass 4: Final Composite
        pipeline.process(
            &ctx.device,
            &ctx.queue,
            &mut encoder,
            &mut targets,
            &uniforms,
            &output_target.view,
            output_format,
        );

        ctx.queue.submit(std::iter::once(encoder.finish()));
        // Poll with Wait to block until GPU execution is completely finished for accurate timing
        ctx.device.poll(wgpu::Maintain::Wait);

        let frame_time = frame_start.elapsed();
        frame_durations.push(frame_time);

        if (frame + 1) % 200 == 0 {
            println!("  Rendered {}/{} frames...", frame + 1, BENCHMARK_FRAMES);
        }
    }

    let total_wall_time = benchmark_start.elapsed();

    // 6. Statistical Computations
    frame_durations.sort();
    let total_render_dur: Duration = frame_durations.iter().sum();
    let mean_ms = (total_render_dur.as_secs_f64() * 1000.0) / (BENCHMARK_FRAMES as f64);
    let min_ms = frame_durations.first().unwrap().as_secs_f64() * 1000.0;
    let max_ms = frame_durations.last().unwrap().as_secs_f64() * 1000.0;
    let p50_ms = frame_durations[BENCHMARK_FRAMES * 50 / 100].as_secs_f64() * 1000.0;
    let p90_ms = frame_durations[BENCHMARK_FRAMES * 90 / 100].as_secs_f64() * 1000.0;
    let p95_ms = frame_durations[BENCHMARK_FRAMES * 95 / 100].as_secs_f64() * 1000.0;
    let p99_ms = frame_durations[BENCHMARK_FRAMES * 99 / 100].as_secs_f64() * 1000.0;
    let effective_fps = 1000.0 / mean_ms;

    println!("\n================ Benchmark Results ================");
    println!(" Target Resolution:    {}x{} (1080p)", TARGET_WIDTH, TARGET_HEIGHT);
    println!(" Total Frames:         {} (after {} warmup frames)", BENCHMARK_FRAMES, WARMUP_FRAMES);
    println!(" Total Benchmark Time: {:.2?}", total_wall_time);
    println!(" Mean Frame Time:      {:.3} ms (Effective FPS: {:.1})", mean_ms, effective_fps);
    println!(" Min Frame Time:       {:.3} ms", min_ms);
    println!(" Max Frame Time:       {:.3} ms", max_ms);
    println!(" Median (p50):         {:.3} ms", p50_ms);
    println!(" 90th Percentile:      {:.3} ms", p90_ms);
    println!(" 95th Percentile:      {:.3} ms", p95_ms);
    println!(" 99th Percentile:      {:.3} ms", p99_ms);
    println!("====================================================");

    // 7. Assert Acceptance Criteria
    if mean_ms <= MAX_ALLOWED_MEAN_MS {
        println!(
            ">>> [PASS] Mean frame time {:.3} ms is below target threshold {:.2} ms (sustained {:.1} FPS >= 60 FPS).",
            mean_ms, MAX_ALLOWED_MEAN_MS, effective_fps
        );
        Ok(())
    } else {
        eprintln!(
            ">>> [FAIL] Mean frame time {:.3} ms EXCEEDED target threshold {:.2} ms!",
            mean_ms, MAX_ALLOWED_MEAN_MS
        );
        std::process::exit(1);
    }
}
