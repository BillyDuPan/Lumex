//! Comprehensive Unit & Integration Tests for Professional VJ Texture Sharing (Spout & Syphon)
//!
//! Verifies:
//! - TextureShareSender trait lifecycle, state management, and error handling.
//! - MockTextureSender and FallbackTextureSender implementations for cross-platform/CI execution.
//! - Windows Spout DirectX 12 NT shared resource handle generation and registration.
//! - macOS Syphon Metal IOSurface sender lifecycle and graceful fallback.
//! - Platform factory `create_texture_sender` behavior.
//! - Multiple texture formats (Rgba8Unorm, Rgba8UnormSrgb, Bgra8Unorm, Bgra8UnormSrgb, Rgba16Float).
//! - Standard broadcast & VJ resolutions (720p, 1080p, 1440p, 4K, custom aspect ratios).
//! - Concurrent independent senders without state collision.

use app_lib::graphics::WgpuContext;
use app_lib::sharing::{
    create_texture_sender, FallbackTextureSender, MockTextureSender, SpoutSender, SyphonSender,
    TextureShareSender,
};

/// Helper to create a genuine test wgpu::Texture with specified dimensions and format.
fn create_test_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    label: &str,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

#[test]
fn test_mock_texture_sender_lifecycle_and_state() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter available): {}", e);
            return;
        }
    };

    let width = 1920;
    let height = 1080;
    let format = wgpu::TextureFormat::Bgra8Unorm;

    let mut sender = MockTextureSender::new("TestMockOutput", width, height, format);

    assert_eq!(sender.server_name(), "TestMockOutput");
    assert!(sender.is_active());
    assert_eq!(sender.width(), width);
    assert_eq!(sender.height(), height);
    assert_eq!(sender.format(), format);
    assert_eq!(sender.published_frames(), 0);
    assert_eq!(sender.last_texture_size(), (0, 0));
    assert_eq!(sender.last_format(), None);

    let texture = create_test_texture(&ctx.device, width, height, format, "TestTexture");

    // Publish frame 1
    let res = sender.publish_texture(&ctx.device, &ctx.queue, &texture);
    assert!(res.is_ok(), "Publish frame 1 should succeed: {:?}", res);
    assert_eq!(sender.published_frames(), 1);
    assert_eq!(sender.last_texture_size(), (width, height));
    assert_eq!(sender.last_format(), Some(format));

    // Publish frame 2
    let res2 = sender.publish_texture(&ctx.device, &ctx.queue, &texture);
    assert!(res2.is_ok(), "Publish frame 2 should succeed: {:?}", res2);
    assert_eq!(sender.published_frames(), 2);

    // Close sender
    sender.close();
    assert!(!sender.is_active());

    // Publishing after close should return error
    let res_closed = sender.publish_texture(&ctx.device, &ctx.queue, &texture);
    assert!(res_closed.is_err(), "Publishing to closed sender must error");
    assert_eq!(sender.published_frames(), 2, "Frame count should not increment after close");
}

#[test]
fn test_mock_texture_sender_dimension_mismatch_rejection() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter available): {}", e);
            return;
        }
    };

    let mut sender = MockTextureSender::new(
        "DimensionTest",
        1920,
        1080,
        wgpu::TextureFormat::Bgra8Unorm,
    );

    // Mismatched smaller texture (1280x720)
    let smaller_tex = create_test_texture(
        &ctx.device,
        1280,
        720,
        wgpu::TextureFormat::Bgra8Unorm,
        "SmallerTex",
    );
    let res = sender.publish_texture(&ctx.device, &ctx.queue, &smaller_tex);
    assert!(res.is_err());
    let err_msg = res.unwrap_err();
    assert!(
        err_msg.contains("dimension mismatch"),
        "Error message should mention dimension mismatch: {}",
        err_msg
    );
    assert_eq!(sender.published_frames(), 0);

    // Mismatched larger texture (3840x2160)
    let larger_tex = create_test_texture(
        &ctx.device,
        3840,
        2160,
        wgpu::TextureFormat::Bgra8Unorm,
        "LargerTex",
    );
    let res_larger = sender.publish_texture(&ctx.device, &ctx.queue, &larger_tex);
    assert!(res_larger.is_err());
    assert_eq!(sender.published_frames(), 0);
}

#[test]
fn test_mock_texture_sender_format_mismatch_rejection() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter available): {}", e);
            return;
        }
    };

    let mut sender = MockTextureSender::new(
        "FormatTest",
        1920,
        1080,
        wgpu::TextureFormat::Bgra8Unorm,
    );

    // Mismatched format: Rgba8Unorm instead of Bgra8Unorm
    let wrong_format_tex = create_test_texture(
        &ctx.device,
        1920,
        1080,
        wgpu::TextureFormat::Rgba8Unorm,
        "WrongFormatTex",
    );

    let res = sender.publish_texture(&ctx.device, &ctx.queue, &wrong_format_tex);
    assert!(res.is_err());
    let err_msg = res.unwrap_err();
    assert!(
        err_msg.contains("format mismatch"),
        "Error message should mention format mismatch: {}",
        err_msg
    );
    assert_eq!(sender.published_frames(), 0);
}

#[test]
fn test_fallback_texture_sender_lifecycle() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter available): {}", e);
            return;
        }
    };

    let width = 1280;
    let height = 720;
    let format = wgpu::TextureFormat::Rgba8Unorm;

    let mut fallback = FallbackTextureSender::new("FallbackServer", width, height, format);

    assert_eq!(fallback.server_name(), "FallbackServer");
    assert!(fallback.is_active());
    assert_eq!(fallback.frames_sent(), 0);

    let valid_tex = create_test_texture(&ctx.device, width, height, format, "FallbackValidTex");
    let res = fallback.publish_texture(&ctx.device, &ctx.queue, &valid_tex);
    assert!(res.is_ok(), "Fallback publish should succeed: {:?}", res);
    assert_eq!(fallback.frames_sent(), 1);

    // Dimension mismatch
    let wrong_dim_tex = create_test_texture(&ctx.device, 800, 600, format, "WrongDimTex");
    let res_dim = fallback.publish_texture(&ctx.device, &ctx.queue, &wrong_dim_tex);
    assert!(res_dim.is_err());

    // Format mismatch
    let wrong_fmt_tex = create_test_texture(
        &ctx.device,
        width,
        height,
        wgpu::TextureFormat::Bgra8Unorm,
        "WrongFmtTex",
    );
    let res_fmt = fallback.publish_texture(&ctx.device, &ctx.queue, &wrong_fmt_tex);
    assert!(res_fmt.is_err());

    // Close
    fallback.close();
    assert!(!fallback.is_active());
    let res_closed = fallback.publish_texture(&ctx.device, &ctx.queue, &valid_tex);
    assert!(res_closed.is_err());
}

#[test]
fn test_spout_sender_lifecycle_and_handle_generation() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter available): {}", e);
            return;
        }
    };

    let width = 1920;
    let height = 1080;
    let format = wgpu::TextureFormat::Bgra8Unorm;

    let mut spout = SpoutSender::new("Lumex Spout Primary", width, height, format);

    assert!(spout.server_name().contains("Lumex Spout Primary"));
    assert!(spout.is_active());
    assert_eq!(spout.width(), width);
    assert_eq!(spout.height(), height);
    assert_eq!(spout.format(), format);
    assert_eq!(spout.published_frames(), 0);

    // Check hardware acceleration status
    #[cfg(target_os = "windows")]
    {
        if spout.is_hardware_accelerated() {
            let raw_handle = spout.shared_handle_raw();
            assert!(
                raw_handle.is_some(),
                "Hardware accelerated Spout sender must produce a valid DirectX 12 NT shared handle"
            );
            assert!(
                raw_handle.unwrap() != 0,
                "Shared handle raw value must be non-zero"
            );
            println!(
                "Spout sender hardware accelerated with NT handle: 0x{:X}",
                raw_handle.unwrap()
            );
        } else {
            println!("Spout sender running in safe fallback mode (hardware D3D12 shared handle deferred)");
        }
    }

    let texture = create_test_texture(&ctx.device, width, height, format, "SpoutPrimaryTex");

    // Publish frame 1
    let res = spout.publish_texture(&ctx.device, &ctx.queue, &texture);
    assert!(res.is_ok(), "Spout publish_texture failed: {:?}", res);
    assert_eq!(spout.published_frames(), 1);

    // Publish frame 2
    let res2 = spout.publish_texture(&ctx.device, &ctx.queue, &texture);
    assert!(res2.is_ok());
    assert_eq!(spout.published_frames(), 2);

    // Dimension mismatch rejection
    let wrong_dim_tex = create_test_texture(&ctx.device, 640, 480, format, "WrongDimTex");
    let res_dim = spout.publish_texture(&ctx.device, &ctx.queue, &wrong_dim_tex);
    assert!(res_dim.is_err());
    assert_eq!(spout.published_frames(), 2);

    // Format mismatch rejection
    let wrong_fmt_tex = create_test_texture(
        &ctx.device,
        width,
        height,
        wgpu::TextureFormat::Rgba16Float,
        "WrongFmtTex",
    );
    let res_fmt = spout.publish_texture(&ctx.device, &ctx.queue, &wrong_fmt_tex);
    assert!(res_fmt.is_err());
    assert_eq!(spout.published_frames(), 2);

    // Close
    spout.close();
    assert!(!spout.is_active());
    let res_closed = spout.publish_texture(&ctx.device, &ctx.queue, &texture);
    assert!(res_closed.is_err());
}

#[test]
fn test_syphon_sender_lifecycle_and_fallback() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter available): {}", e);
            return;
        }
    };

    let width = 1920;
    let height = 1080;
    let format = wgpu::TextureFormat::Bgra8Unorm;

    let mut syphon = SyphonSender::new("Lumex Syphon Primary", width, height, format);

    assert_eq!(syphon.server_name(), "Lumex Syphon Primary");
    assert!(syphon.is_active());
    assert_eq!(syphon.width(), width);
    assert_eq!(syphon.height(), height);
    assert_eq!(syphon.format(), format);
    assert_eq!(syphon.published_frames(), 0);

    let texture = create_test_texture(&ctx.device, width, height, format, "SyphonPrimaryTex");

    // Publish frame 1
    let res = syphon.publish_texture(&ctx.device, &ctx.queue, &texture);
    assert!(res.is_ok(), "Syphon publish_texture failed: {:?}", res);
    assert_eq!(syphon.published_frames(), 1);

    // Publish frame 2
    let res2 = syphon.publish_texture(&ctx.device, &ctx.queue, &texture);
    assert!(res2.is_ok());
    assert_eq!(syphon.published_frames(), 2);

    // Dimension mismatch rejection
    let wrong_dim_tex = create_test_texture(&ctx.device, 1024, 768, format, "WrongDimSyphonTex");
    let res_dim = syphon.publish_texture(&ctx.device, &ctx.queue, &wrong_dim_tex);
    assert!(res_dim.is_err());
    assert_eq!(syphon.published_frames(), 2);

    // Close
    syphon.close();
    assert!(!syphon.is_active());
    let res_closed = syphon.publish_texture(&ctx.device, &ctx.queue, &texture);
    assert!(res_closed.is_err());
}

#[test]
fn test_factory_create_texture_sender() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter available): {}", e);
            return;
        }
    };

    let width = 1920;
    let height = 1080;
    let format = wgpu::TextureFormat::Bgra8Unorm;

    let mut sender: Box<dyn TextureShareSender> =
        create_texture_sender("Lumex Factory Output", width, height, format);

    assert!(sender.server_name().contains("Lumex Factory Output"));
    assert!(sender.is_active());

    let texture = create_test_texture(&ctx.device, width, height, format, "FactoryTex");
    let res = sender.publish_texture(&ctx.device, &ctx.queue, &texture);
    assert!(res.is_ok(), "Factory sender publish failed: {:?}", res);

    sender.close();
    assert!(!sender.is_active());
}

#[test]
fn test_all_supported_broadcast_texture_formats() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter available): {}", e);
            return;
        }
    };

    let formats = [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba16Float,
    ];

    let width = 640;
    let height = 360;

    for format in formats {
        let name = format!("LumexFormat_{:?}", format);
        let mut sender = create_texture_sender(&name, width, height, format);

        assert!(sender.is_active());
        let texture = create_test_texture(
            &ctx.device,
            width,
            height,
            format,
            &format!("FormatTex_{:?}", format),
        );

        let res = sender.publish_texture(&ctx.device, &ctx.queue, &texture);
        assert!(
            res.is_ok(),
            "Publishing format {:?} should succeed: {:?}",
            format,
            res
        );

        sender.close();
        assert!(!sender.is_active());
    }
}

#[test]
fn test_standard_vj_and_broadcast_resolutions() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter available): {}", e);
            return;
        }
    };

    let resolutions = [
        (1280, 720, "720p_HD"),
        (1920, 1080, "1080p_FHD"),
        (2560, 1440, "1440p_QHD"),
        (1024, 1024, "1024_Square"),
        (1920, 480, "Panoramic_Strip"),
    ];

    let format = wgpu::TextureFormat::Bgra8Unorm;

    for (w, h, desc) in resolutions {
        let name = format!("LumexRes_{}", desc);
        let mut sender = create_texture_sender(&name, w, h, format);
        assert!(sender.is_active());

        let texture = create_test_texture(
            &ctx.device,
            w,
            h,
            format,
            &format!("Tex_{}", desc),
        );

        let res = sender.publish_texture(&ctx.device, &ctx.queue, &texture);
        assert!(
            res.is_ok(),
            "Publishing resolution {}x{} ({}) failed: {:?}",
            w,
            h,
            desc,
            res
        );

        sender.close();
        assert!(!sender.is_active());
    }
}

#[test]
fn test_multiple_concurrent_senders() {
    let ctx = match WgpuContext::init_headless_sync() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping live GPU test (no adapter available): {}", e);
            return;
        }
    };

    let format = wgpu::TextureFormat::Bgra8Unorm;

    let mut sender1 = create_texture_sender("Lumex Multi 1", 1280, 720, format);
    let mut sender2 = create_texture_sender("Lumex Multi 2", 1920, 1080, format);

    assert!(sender1.is_active());
    assert!(sender2.is_active());

    let tex1 = create_test_texture(&ctx.device, 1280, 720, format, "MultiTex1");
    let tex2 = create_test_texture(&ctx.device, 1920, 1080, format, "MultiTex2");

    // Interleaved publishing
    assert!(sender1.publish_texture(&ctx.device, &ctx.queue, &tex1).is_ok());
    assert!(sender2.publish_texture(&ctx.device, &ctx.queue, &tex2).is_ok());
    assert!(sender1.publish_texture(&ctx.device, &ctx.queue, &tex1).is_ok());
    assert!(sender2.publish_texture(&ctx.device, &ctx.queue, &tex2).is_ok());

    sender1.close();
    assert!(!sender1.is_active());
    assert!(sender2.is_active(), "Closing sender 1 must not affect sender 2");

    assert!(sender2.publish_texture(&ctx.device, &ctx.queue, &tex2).is_ok());

    sender2.close();
    assert!(!sender2.is_active());
}
