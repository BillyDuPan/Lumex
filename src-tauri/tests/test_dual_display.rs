//! Integration and Unit Tests for Milestone 2: Dual Display & Zero-Copy Preview
//!
//! Tests:
//! 1. Display enumeration, metadata parsing, and monitor config mapping.
//! 2. Target monitor selection bounds, secondary display prioritization, aspect ratios.
//! 3. Projector configuration, borderless fullscreen mode, and hardware V-Sync.
//! 4. Zero-copy GPU blit pipeline compilation, format caching, and execution.
//! 5. Dual-surface simultaneous presentation on shared WGPU Device and Queue.
//! 6. Viewport fitting math (letterbox, pillarbox, extreme aspect ratios, 0x0 clamps).
//! 7. PreviewSurface resize bounds and viewport updates.
//! 8. Engine state machine transitions, mock projector lifecycle, and status querying.

use app_lib::display::{
    DisplayManager, DualPresentationResult, DualSurfacePresenter, EngineState, GpuBlitPipeline,
    MonitorInfo, PreviewSurface, ProjectorConfig, ProjectorControl, ProjectorStatus, ViewportRect,
};
use app_lib::graphics::{RenderTarget, WgpuContext};
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn test_display_manager_monitor_mapping_and_prioritization() {
    // 1. Test empty monitor list defaults
    let dm_empty = DisplayManager::new();
    assert_eq!(dm_empty.monitor_count(), 0);
    assert!(dm_empty.get_selected().is_none());
    assert_eq!(dm_empty.secondary_monitors().len(), 0);

    // 2. Test multi-monitor configuration
    let monitors = vec![
        MonitorInfo {
            index: 0,
            id: "display-0".into(),
            name: "Built-in Laptop Display".into(),
            primary: true,
            width: 1920,
            height: 1080,
            x: 0,
            y: 0,
            scale_factor: 1.25,
            refresh_rate_millihertz: Some(60_000),
        },
        MonitorInfo {
            index: 1,
            id: "display-1".into(),
            name: "HDMI Beamer 4K".into(),
            primary: false,
            width: 3840,
            height: 2160,
            x: 1920,
            y: 0,
            scale_factor: 1.0,
            refresh_rate_millihertz: Some(120_000),
        },
        MonitorInfo {
            index: 2,
            id: "display-2".into(),
            name: "DisplayPort Stage LED".into(),
            primary: false,
            width: 1920,
            height: 1200,
            x: 5760,
            y: 0,
            scale_factor: 1.0,
            refresh_rate_millihertz: Some(144_000),
        },
    ];

    let mut dm = DisplayManager::from_monitors(monitors);
    assert_eq!(dm.monitor_count(), 3);

    // Automatic default should prioritize the first secondary monitor (HDMI Beamer, index 1)
    assert_eq!(dm.selected_index(), 1);
    let selected = dm.get_selected().expect("Selected monitor should exist");
    assert_eq!(selected.name, "HDMI Beamer 4K");
    assert_eq!(selected.width, 3840);
    assert_eq!(selected.height, 2160);
    assert!(!selected.primary);

    // Primary monitor lookup
    let primary = dm.primary_monitor().expect("Primary monitor must exist");
    assert_eq!(primary.index, 0);
    assert!(primary.primary);

    // Secondary monitors list
    let secondaries = dm.secondary_monitors();
    assert_eq!(secondaries.len(), 2);
    assert_eq!(secondaries[0].index, 1);
    assert_eq!(secondaries[1].index, 2);

    // Switch selection to stage LED
    dm.select_monitor(2).expect("Selection index 2 should succeed");
    assert_eq!(dm.selected_index(), 2);
    assert_eq!(dm.get_selected().unwrap().name, "DisplayPort Stage LED");

    // Out-of-bounds selection must return Err
    assert!(dm.select_monitor(99).is_err());
    // Selection should remain unchanged on error
    assert_eq!(dm.selected_index(), 2);
}

#[test]
fn test_monitor_aspect_ratio_and_geometry() {
    let m_16_9 = MonitorInfo {
        index: 0,
        id: "d0".into(),
        name: "FHD".into(),
        primary: true,
        width: 1920,
        height: 1080,
        x: 0,
        y: 0,
        scale_factor: 1.0,
        refresh_rate_millihertz: None,
    };
    assert!((m_16_9.aspect_ratio() - (16.0 / 9.0)).abs() < 1e-4);
    assert!(m_16_9.is_landscape());

    let m_ultrawide = MonitorInfo {
        index: 1,
        id: "d1".into(),
        name: "21:9".into(),
        primary: false,
        width: 3440,
        height: 1440,
        x: 1920,
        y: 0,
        scale_factor: 1.0,
        refresh_rate_millihertz: None,
    };
    assert!((m_ultrawide.aspect_ratio() - (3440.0 / 1440.0)).abs() < 1e-4);
    assert!(m_ultrawide.is_landscape());

    let m_portrait = MonitorInfo {
        index: 2,
        id: "d2".into(),
        name: "Portrait".into(),
        primary: false,
        width: 1080,
        height: 1920,
        x: 5360,
        y: 0,
        scale_factor: 1.0,
        refresh_rate_millihertz: None,
    };
    assert!(!m_portrait.is_landscape());

    // Degenerate zero height safely defaults to 1.0 aspect ratio
    let m_zero = MonitorInfo {
        index: 3,
        id: "d3".into(),
        name: "Zero".into(),
        primary: false,
        width: 1920,
        height: 0,
        x: 0,
        y: 0,
        scale_factor: 1.0,
        refresh_rate_millihertz: None,
    };
    assert_eq!(m_zero.aspect_ratio(), 1.0);
}

#[test]
fn test_projector_config_and_status_serialization() {
    let cfg = ProjectorConfig {
        title: "Test Projector".into(),
        monitor_index: 1,
        borderless_fullscreen: true,
        always_on_top: true,
        vsync: true,
    };
    assert_eq!(cfg.title, "Test Projector");
    assert_eq!(cfg.monitor_index, 1);
    assert!(cfg.borderless_fullscreen);
    assert!(cfg.always_on_top);
    assert!(cfg.vsync);

    let status = ProjectorStatus {
        is_open: true,
        is_fullscreen: true,
        monitor_index: 1,
        monitor_name: "Optoma Beamer 1080p".into(),
        resolution: [1920, 1080],
    };

    let json = serde_json::to_string(&status).expect("Serialization to JSON must succeed");
    let deserialized: ProjectorStatus =
        serde_json::from_str(&json).expect("Deserialization from JSON must succeed");
    assert_eq!(status, deserialized);
}

#[test]
fn test_viewport_fitting_letterbox_pillarbox_and_clamping() {
    // 16:9 source in exactly matching 16:9 container
    let exact = ViewportRect::fit_aspect_ratio(16.0 / 9.0, 1920.0, 1080.0);
    assert_eq!(exact.x, 0.0);
    assert_eq!(exact.y, 0.0);
    assert_eq!(exact.width, 1920.0);
    assert_eq!(exact.height, 1080.0);

    // 16:9 source in wider 2560x1080 container -> pillarbox (black bars on left/right)
    let wide = ViewportRect::fit_aspect_ratio(16.0 / 9.0, 2560.0, 1080.0);
    assert_eq!(wide.height, 1080.0);
    assert_eq!(wide.width, 1920.0);
    assert_eq!(wide.x, (2560.0 - 1920.0) / 2.0); // 320.0
    assert_eq!(wide.y, 0.0);

    // 16:9 source in taller 1080x1920 container -> letterbox (black bars on top/bottom)
    let tall = ViewportRect::fit_aspect_ratio(16.0 / 9.0, 1080.0, 1920.0);
    let expected_h = 1080.0 / (16.0 / 9.0);
    assert_eq!(tall.width, 1080.0);
    assert!((tall.height - expected_h).abs() < 1e-4);
    assert_eq!(tall.x, 0.0);
    assert!((tall.y - (1920.0 - expected_h) / 2.0).abs() < 1e-4);

    // Extreme zero / negative container dimensions clamped safely
    let zero = ViewportRect::fit_aspect_ratio(16.0 / 9.0, 0.0, 0.0);
    assert!(zero.width >= 1.0);
    assert!(zero.height >= 1.0);
    assert!(zero.x >= 0.0);
    assert!(zero.y >= 0.0);
}

#[test]
fn test_preview_surface_resize_and_viewport_bounds() {
    let mut prev = PreviewSurface::new(640, 360, wgpu::TextureFormat::Rgba8UnormSrgb);
    assert_eq!(prev.width, 640);
    assert_eq!(prev.height, 360);
    assert_eq!(prev.format, wgpu::TextureFormat::Rgba8UnormSrgb);

    // Update viewport rect
    let vp = ViewportRect::new(10.0, 20.0, 620.0, 320.0);
    prev.update_viewport(vp);
    assert_eq!(prev.viewport, vp);

    // Zero dimension clamps safely to >= 1x1
    let zero_surf = PreviewSurface::new(0, 0, wgpu::TextureFormat::Bgra8UnormSrgb);
    assert_eq!(zero_surf.width, 1);
    assert_eq!(zero_surf.height, 1);
}

#[test]
fn test_gpu_blit_pipeline_creation_and_execution() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    // 1. Create blit pipeline for standard swapchain format
    let mut blit_pipeline =
        GpuBlitPipeline::new(&ctx.device, wgpu::TextureFormat::Rgba8UnormSrgb);

    // 2. Test pipeline caching across different target formats
    let _pipe_bgra =
        blit_pipeline.get_or_create_pipeline(&ctx.device, wgpu::TextureFormat::Bgra8UnormSrgb);
    let _pipe_hdr =
        blit_pipeline.get_or_create_pipeline(&ctx.device, wgpu::TextureFormat::Rgba16Float);

    assert!(blit_pipeline
        .pipelines
        .contains_key(&wgpu::TextureFormat::Rgba8UnormSrgb));
    assert!(blit_pipeline
        .pipelines
        .contains_key(&wgpu::TextureFormat::Bgra8UnormSrgb));
    assert!(blit_pipeline
        .pipelines
        .contains_key(&wgpu::TextureFormat::Rgba16Float));

    // 3. Allocate offscreen scene texture (RGBA16F master render target)
    let scene_target = RenderTarget::new(
        &ctx.device,
        1920,
        1080,
        wgpu::TextureFormat::Rgba16Float,
        Some("MasterSceneTarget"),
    );

    // 4. Allocate simulated destination swapchain target (RGBA8)
    let swapchain_sim = RenderTarget::new(
        &ctx.device,
        1280,
        720,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Some("SwapchainSimTarget"),
    );

    // 5. Execute GPU blit
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("TestBlitEncoder"),
        });

    let vp = ViewportRect::new(0.0, 0.0, 1280.0, 720.0);
    blit_pipeline.blit(
        &ctx.device,
        &mut encoder,
        &scene_target.view,
        &swapchain_sim.view,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Some(&vp),
    );

    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);
}

#[test]
fn test_dual_surface_simultaneous_gpu_blits() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    let mut blit_pipeline =
        GpuBlitPipeline::new(&ctx.device, wgpu::TextureFormat::Rgba8UnormSrgb);

    // Source master visual texture (RGBA16F offscreen post-processing output)
    let master_source = RenderTarget::new(
        &ctx.device,
        1920,
        1080,
        wgpu::TextureFormat::Rgba16Float,
        Some("MasterSource"),
    );

    // Target 1: Native Projector Output (e.g. 4K 3840x2160 Bgra8UnormSrgb)
    let projector_target = RenderTarget::new(
        &ctx.device,
        3840,
        2160,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        Some("SimulatedProjectorSurface"),
    );

    // Target 2: Control UI Preview (e.g. 640x360 Rgba8UnormSrgb with fitted letterbox)
    let preview_target = RenderTarget::new(
        &ctx.device,
        640,
        360,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Some("SimulatedPreviewSurface"),
    );

    // Single unified command encoder for both GPU blits — zero CPU copies
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SimultaneousDualBlitEncoder"),
        });

    // Blit to Projector (fullscreen)
    blit_pipeline.blit(
        &ctx.device,
        &mut encoder,
        &master_source.view,
        &projector_target.view,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        None,
    );

    // Blit to Preview (with aspect ratio fitting)
    let preview_vp = ViewportRect::fit_aspect_ratio(16.0 / 9.0, 640.0, 360.0);
    blit_pipeline.blit(
        &ctx.device,
        &mut encoder,
        &master_source.view,
        &preview_target.view,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        Some(&preview_vp),
    );

    ctx.queue.submit(std::iter::once(encoder.finish()));
    ctx.device.poll(wgpu::Maintain::Wait);
}

#[test]
fn test_dual_surface_presenter_api() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter): {}", e);
            return;
        }
    };

    let mut presenter =
        DualSurfacePresenter::new(&ctx.device, wgpu::TextureFormat::Rgba8UnormSrgb);

    let dummy_source = RenderTarget::new(
        &ctx.device,
        1920,
        1080,
        wgpu::TextureFormat::Rgba16Float,
        Some("DummySource"),
    );

    // Present with no active surfaces
    let res = presenter
        .present_dual(
            &ctx.device,
            &ctx.queue,
            &dummy_source.view,
            16.0 / 9.0,
            None,
            None,
        )
        .expect("Presentation with no surfaces should succeed cleanly");

    assert_eq!(
        res,
        DualPresentationResult {
            projector_presented: false,
            preview_presented: false,
        }
    );
}

/// Mock ProjectorControl implementation for testing state machine transitions
struct MockProjector {
    is_open: AtomicBool,
    is_fullscreen: AtomicBool,
    resolution: (u32, u32),
    monitor_index: usize,
    monitor_name: String,
}

impl MockProjector {
    fn new(monitor_index: usize, name: &str, res: (u32, u32)) -> Self {
        Self {
            is_open: AtomicBool::new(true),
            is_fullscreen: AtomicBool::new(true),
            resolution: res,
            monitor_index,
            monitor_name: name.to_string(),
        }
    }
}

impl ProjectorControl for MockProjector {
    fn is_open(&self) -> bool {
        self.is_open.load(Ordering::SeqCst)
    }

    fn is_fullscreen(&self) -> bool {
        self.is_fullscreen.load(Ordering::SeqCst)
    }

    fn resolution(&self) -> (u32, u32) {
        self.resolution
    }

    fn toggle_fullscreen(&self) -> Result<bool, String> {
        let cur = self.is_fullscreen.load(Ordering::SeqCst);
        let next = !cur;
        self.is_fullscreen.store(next, Ordering::SeqCst);
        Ok(next)
    }

    fn close(&self) -> Result<(), String> {
        self.is_open.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn monitor_name(&self) -> String {
        self.monitor_name.clone()
    }

    fn monitor_index(&self) -> usize {
        self.monitor_index
    }
}

#[test]
fn test_engine_state_lifecycle_and_status() {
    let state = match EngineState::new_headless() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Skipping engine state test (no GPU adapter): {}", e);
            return;
        }
    };

    // 1. Initial idle status
    let status = state.get_status();
    assert!(!status.projector_active);
    assert!(!status.preview_active);
    assert!(!status.fullscreen);
    assert_eq!(status.vsync_mode, "Fifo");
    assert_eq!(status.frame_count, 0);

    // 2. Closing projector when not open is idempotent
    let close_result = state.close_projector().expect("Close projector should succeed");
    assert!(!close_result.is_open);

    // 3. Toggle fullscreen without open projector should return Err
    assert!(state.toggle_projector_fullscreen().is_err());

    // 4. Attach mock projector and verify state transitions
    let mock = MockProjector::new(1, "Barco Event Projector 4K", (3840, 2160));
    {
        let mut proj_lock = state.projector.lock().unwrap();
        *proj_lock = Some(Box::new(mock));
    }

    let active_status = state.get_status();
    assert!(active_status.projector_active);
    assert!(active_status.fullscreen);
    assert_eq!(active_status.surface_resolution, Some([3840, 2160]));
    let cur_mon = active_status.current_monitor.expect("Monitor should exist");
    assert_eq!(cur_mon.index, 1);
    assert_eq!(cur_mon.name, "Barco Event Projector 4K");

    // 5. Toggle fullscreen
    let new_fs = state.toggle_projector_fullscreen().expect("Toggle fullscreen must succeed");
    assert!(!new_fs);
    assert!(!state.get_status().fullscreen);

    let new_fs_again = state.toggle_projector_fullscreen().expect("Toggle fullscreen again");
    assert!(new_fs_again);
    assert!(state.get_status().fullscreen);

    // 6. Close projector
    let closed = state.close_projector().expect("Close must succeed");
    assert!(!closed.is_open);
    assert_eq!(closed.monitor_index, 1);
    assert_eq!(closed.monitor_name, "Barco Event Projector 4K");

    let post_close = state.get_status();
    assert!(!post_close.projector_active);
    assert!(!post_close.fullscreen);
}
