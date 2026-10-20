// Headless Naga WGSL Shader Validation Test Suite

#[test]
fn test_all_shaders_compile() {
    let shaders = [
        ("echo.wgsl", include_str!("../src/shaders/echo.wgsl")),
        ("downsample13.wgsl", include_str!("../src/shaders/downsample13.wgsl")),
        ("blur_glow_comp.wgsl", include_str!("../src/shaders/blur_glow_comp.wgsl")),
        ("final_composite.wgsl", include_str!("../src/shaders/final_composite.wgsl")),
    ];

    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );

    for (name, source) in shaders {
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("Failed to parse WGSL shader {}: {:?}", name, e));

        let _info = validator.validate(&module)
            .unwrap_or_else(|e| panic!("Failed to validate WGSL shader {}: {:?}", name, e));

        println!("Successfully validated {}", name);
    }
}

#[test]
fn test_wgpu_pipeline_execution() {
    let ctx = match app_lib::graphics::WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU execution (no adapter available): {}", e);
            return;
        }
    };

    let width = 640;
    let height = 360;
    let mut targets = app_lib::graphics::PostProcessTargets::new(&ctx.device, width, height);
    let mut pipeline = app_lib::graphics::PostProcessPipeline::new(&ctx.device, wgpu::TextureFormat::Rgba8UnormSrgb);

    let output_target = app_lib::graphics::RenderTarget::new(
        &ctx.device,
        width,
        height,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Some("TestOutputTarget"),
    );

    let mut state_machine = app_lib::graphics::FxStateMachine::new();
    state_machine.target.echo = 0.5;
    state_machine.target.blur = 0.5;
    state_machine.target.glow = 0.8;
    state_machine.target.warp = 0.04;
    state_machine.target.k_n = 6.0;
    state_machine.step(0.016);

    let uniforms = state_machine.build_uniforms(width, height, 0.016);

    let mut encoder = ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("TestCommandEncoder"),
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
    println!("Successfully executed WGPU 4-pass post-processing pipeline on real GPU device!");
}
