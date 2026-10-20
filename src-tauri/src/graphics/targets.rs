//! Lumex Render Targets & Texture Pyramid
//!
//! Offscreen floating-point render targets, double-buffered ping-pong echo buffers,
//! and 4-level downsampling blur pyramid.

pub const BLUR_LEVELS: usize = 4;

#[derive(Debug)]
pub struct RenderTarget {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
}

impl RenderTarget {
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        label: Option<&str>,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            width,
            height,
            format,
        }
    }

    pub fn texel_size(&self) -> [f32; 2] {
        [1.0 / self.width as f32, 1.0 / self.height as f32]
    }
}

/// Double-buffered ping-pong echo textures for feedback accumulation.
/// Uses Rgba16Float to ensure faint trails decay smoothly without 8-bit quantization banding.
#[derive(Debug)]
pub struct EchoTargets {
    pub targets: [RenderTarget; 2],
    pub current_idx: usize,
    pub is_warm: bool,
}

impl EchoTargets {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let t0 = RenderTarget::new(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba16Float,
            Some("EchoPingPong0"),
        );
        let t1 = RenderTarget::new(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba16Float,
            Some("EchoPingPong1"),
        );

        Self {
            targets: [t0, t1],
            current_idx: 0,
            is_warm: false,
        }
    }

    pub fn current(&self) -> &RenderTarget {
        &self.targets[self.current_idx]
    }

    pub fn next(&self) -> &RenderTarget {
        &self.targets[1 - self.current_idx]
    }

    pub fn swap(&mut self) {
        self.current_idx = 1 - self.current_idx;
        self.is_warm = true;
    }

    pub fn reset(&mut self) {
        self.is_warm = false;
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.targets[0] = RenderTarget::new(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba16Float,
            Some("EchoPingPong0"),
        );
        self.targets[1] = RenderTarget::new(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba16Float,
            Some("EchoPingPong1"),
        );
        self.current_idx = 0;
        self.is_warm = false;
    }
}

/// 4-level downsampling blur pyramid.
/// Level 1: W/2 x H/2
/// Level 2: W/4 x H/4
/// Level 3: W/8 x H/8
/// Level 4: W/16 x H/16
#[derive(Debug)]
pub struct BlurPyramid {
    pub levels: Vec<RenderTarget>,
}

impl BlurPyramid {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let mut levels = Vec::with_capacity(BLUR_LEVELS);
        for i in 1..=BLUR_LEVELS {
            let divisor = 1u32 << i;
            let lw = (width / divisor).max(2);
            let lh = (height / divisor).max(2);
            let label = format!("BlurPyramidLevel{}", i);
            levels.push(RenderTarget::new(
                device,
                lw,
                lh,
                wgpu::TextureFormat::Rgba16Float,
                Some(&label),
            ));
        }

        Self { levels }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        *self = Self::new(device, width, height);
    }
}

/// Unified render target container managing all offscreen post-processing targets.
#[derive(Debug)]
pub struct PostProcessTargets {
    pub width: u32,
    pub height: u32,
    pub src_target: RenderTarget,
    pub echo_targets: EchoTargets,
    pub pyramid: BlurPyramid,
    pub comp_target: RenderTarget,
}

impl PostProcessTargets {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let src_target = RenderTarget::new(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba16Float,
            Some("SceneSourceTarget"),
        );

        let echo_targets = EchoTargets::new(device, width, height);
        let pyramid = BlurPyramid::new(device, width, height);
        let comp_target = RenderTarget::new(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba16Float,
            Some("BlurGlowCompTarget"),
        );

        Self {
            width,
            height,
            src_target,
            echo_targets,
            pyramid,
            comp_target,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        self.src_target = RenderTarget::new(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba16Float,
            Some("SceneSourceTarget"),
        );
        self.echo_targets.resize(device, width, height);
        self.pyramid.resize(device, width, height);
        self.comp_target = RenderTarget::new(
            device,
            width,
            height,
            wgpu::TextureFormat::Rgba16Float,
            Some("BlurGlowCompTarget"),
        );
    }
}
