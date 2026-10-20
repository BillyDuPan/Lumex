//! Embedded GPU Preview Surface & Zero-Copy Dual Presentation
//!
//! Provides direct embedded GPU preview blitting in the Tauri control window
//! and zero-copy dual-surface presentation using shared WGPU Device and Queue.
//! Visual scene is rendered once into offscreen PostProcessTargets, then blitted
//! directly to both preview and output surfaces purely in VRAM without CPU copies.

use std::collections::HashMap;
use std::sync::Arc;

use crate::display::projector::ProjectorSurface;

/// Bounding rectangle for preview viewport inside the control window.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ViewportRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ViewportRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x: x.max(0.0),
            y: y.max(0.0),
            width: width.max(1.0),
            height: height.max(1.0),
        }
    }

    /// Calculate fitted letterbox/pillarbox viewport inside a destination container.
    pub fn fit_aspect_ratio(source_aspect: f32, container_w: f32, container_h: f32) -> Self {
        let safe_aspect = if source_aspect > 0.001 {
            source_aspect
        } else {
            16.0 / 9.0
        };
        let c_w = container_w.max(1.0);
        let c_h = container_h.max(1.0);
        let container_aspect = c_w / c_h;

        if container_aspect > safe_aspect {
            // Container is wider than content -> pillarbox (bars on left/right)
            let fitted_w = c_h * safe_aspect;
            let offset_x = (c_w - fitted_w) * 0.5;
            Self::new(offset_x, 0.0, fitted_w, c_h)
        } else {
            // Container is taller than content -> letterbox (bars on top/bottom)
            let fitted_h = c_w / safe_aspect;
            let offset_y = (c_h - fitted_h) * 0.5;
            Self::new(0.0, offset_y, c_w, fitted_h)
        }
    }
}

/// GPU blit pipeline for zero-copy texture transfers between offscreen targets and swapchains.
pub struct GpuBlitPipeline {
    pub sampler: wgpu::Sampler,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub pipeline_layout: wgpu::PipelineLayout,
    pub shader_module: Arc<wgpu::ShaderModule>,
    pub pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
}

impl GpuBlitPipeline {
    pub const SHADER_SRC: &'static str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = -1.0 + f32((vertex_index & 1u) << 2u);
    let y = -1.0 + f32((vertex_index & 2u) << 1u);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@group(0) @binding(0) var u_sampler: sampler;
@group(0) @binding(1) var u_texture: texture_2d<f32>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(u_texture, u_sampler, in.uv);
}
"#;

    pub fn new(device: &wgpu::Device, default_format: wgpu::TextureFormat) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("BlitSampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("BlitBindGroupLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("BlitPipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader_module = Arc::new(device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BlitShader"),
            source: wgpu::ShaderSource::Wgsl(Self::SHADER_SRC.into()),
        }));

        let mut pipelines = HashMap::new();
        let default_pipe = Self::build_pipeline(device, &pipeline_layout, &shader_module, default_format);
        pipelines.insert(default_format, default_pipe);

        Self {
            sampler,
            bind_group_layout,
            pipeline_layout,
            shader_module,
            pipelines,
        }
    }

    fn build_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("BlitPipeline_{:?}", format)),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    pub fn get_or_create_pipeline(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.pipelines.entry(format).or_insert_with(|| {
            Self::build_pipeline(device, &self.pipeline_layout, &self.shader_module, format)
        })
    }

    /// Execute GPU blit from source texture view to destination view.
    pub fn blit(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
        destination_view: &wgpu::TextureView,
        destination_format: wgpu::TextureFormat,
        viewport: Option<&ViewportRect>,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BlitBindGroup"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(source_view),
                },
            ],
        });

        let pipeline = self.get_or_create_pipeline(device, destination_format);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("GpuBlitPass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: destination_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    // Dark background (#161719) matching Lumex control UI
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.086,
                        g: 0.090,
                        b: 0.098,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if let Some(vp) = viewport {
            pass.set_viewport(vp.x, vp.y, vp.width, vp.height, 0.0, 1.0);
            pass.set_scissor_rect(
                vp.x as u32,
                vp.y as u32,
                vp.width as u32,
                vp.height as u32,
            );
        }

        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Embedded GPU preview surface inside the control window.
pub struct PreviewSurface {
    pub surface: Option<wgpu::Surface<'static>>,
    pub config: Option<wgpu::SurfaceConfiguration>,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    pub viewport: ViewportRect,
}

impl PreviewSurface {
    pub fn new(width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        let safe_w = width.max(1);
        let safe_h = height.max(1);
        Self {
            surface: None,
            config: None,
            width: safe_w,
            height: safe_h,
            format,
            viewport: ViewportRect::new(0.0, 0.0, safe_w as f32, safe_h as f32),
        }
    }

    /// Attach a native surface to this preview state and configure it on the GPU device.
    pub fn attach_surface(
        &mut self,
        device: &wgpu::Device,
        adapter: &wgpu::Adapter,
        surface: wgpu::Surface<'static>,
    ) -> Result<(), String> {
        let caps = surface.get_capabilities(adapter);
        if caps.formats.is_empty() {
            return Err("Preview surface has no supported formats".to_string());
        }

        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| *f == wgpu::TextureFormat::Bgra8UnormSrgb || *f == wgpu::TextureFormat::Rgba8UnormSrgb)
            .unwrap_or(caps.formats[0]);

        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
            wgpu::PresentMode::Fifo
        } else {
            caps.present_modes[0]
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: self.width,
            height: self.height,
            present_mode,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(device, &config);
        self.format = format;
        self.config = Some(config);
        self.surface = Some(surface);

        Ok(())
    }

    /// Resize preview surface dimensions.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let safe_w = width.max(1);
        let safe_h = height.max(1);
        if self.width != safe_w || self.height != safe_h {
            self.width = safe_w;
            self.height = safe_h;
            if let (Some(surf), Some(cfg)) = (&self.surface, &mut self.config) {
                cfg.width = safe_w;
                cfg.height = safe_h;
                surf.configure(device, cfg);
            }
        }
    }

    /// Update preview DOM container viewport bounds.
    pub fn update_viewport(&mut self, rect: ViewportRect) {
        self.viewport = rect;
    }
}

/// Results of a dual-surface presentation cycle.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct DualPresentationResult {
    pub projector_presented: bool,
    pub preview_presented: bool,
}

/// Dual-surface presenter executing GPU blits to both outputs in a single pass.
pub struct DualSurfacePresenter {
    pub blit_pipeline: GpuBlitPipeline,
}

impl DualSurfacePresenter {
    pub fn new(device: &wgpu::Device, default_format: wgpu::TextureFormat) -> Self {
        Self {
            blit_pipeline: GpuBlitPipeline::new(device, default_format),
        }
    }

    /// Present a single offscreen source texture view to both projector and preview surfaces
    /// on the GPU with zero CPU memory copies.
    pub fn present_dual(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source_view: &wgpu::TextureView,
        source_aspect: f32,
        projector_surface: Option<&mut ProjectorSurface>,
        preview_surface: Option<&mut PreviewSurface>,
    ) -> Result<DualPresentationResult, String> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("DualSurfaceBlitEncoder"),
        });

        let mut proj_presented = false;
        let mut prev_presented = false;

        let mut proj_frame = None;
        if let Some(proj) = projector_surface {
            match proj.surface.get_current_texture() {
                Ok(frame) => {
                    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                    self.blit_pipeline.blit(
                        device,
                        &mut encoder,
                        source_view,
                        &view,
                        proj.format,
                        None, // Fullscreen on projector
                    );
                    proj_frame = Some(frame);
                    proj_presented = true;
                }
                Err(e) => {
                    log::warn!("Projector surface get_current_texture error: {}", e);
                }
            }
        }

        let mut prev_frame = None;
        if let Some(prev) = preview_surface {
            if let Some(ref surface) = prev.surface {
                match surface.get_current_texture() {
                    Ok(frame) => {
                        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                        let fitted_vp = ViewportRect::fit_aspect_ratio(
                            source_aspect,
                            prev.width as f32,
                            prev.height as f32,
                        );
                        self.blit_pipeline.blit(
                            device,
                            &mut encoder,
                            source_view,
                            &view,
                            prev.format,
                            Some(&fitted_vp),
                        );
                        prev_frame = Some(frame);
                        prev_presented = true;
                    }
                    Err(e) => {
                        log::warn!("Preview surface get_current_texture error: {}", e);
                    }
                }
            }
        }

        queue.submit(std::iter::once(encoder.finish()));

        if let Some(frame) = proj_frame {
            frame.present();
        }
        if let Some(frame) = prev_frame {
            frame.present();
        }

        Ok(DualPresentationResult {
            projector_presented: proj_presented,
            preview_presented: prev_presented,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_rect_clamps_and_aspect_ratio_fitting() {
        // Zero dimensions clamped safely
        let r0 = ViewportRect::new(-10.0, -5.0, 0.0, 0.0);
        assert_eq!(r0.x, 0.0);
        assert_eq!(r0.y, 0.0);
        assert_eq!(r0.width, 1.0);
        assert_eq!(r0.height, 1.0);

        // 16:9 source in 1920x1080 container -> exact fit
        let r_exact = ViewportRect::fit_aspect_ratio(16.0 / 9.0, 1920.0, 1080.0);
        assert_eq!(r_exact.x, 0.0);
        assert_eq!(r_exact.y, 0.0);
        assert_eq!(r_exact.width, 1920.0);
        assert_eq!(r_exact.height, 1080.0);

        // 16:9 source in ultrawide 2560x1080 container (ratio > 16:9) -> pillarbox
        let r_wide = ViewportRect::fit_aspect_ratio(16.0 / 9.0, 2560.0, 1080.0);
        assert_eq!(r_wide.height, 1080.0);
        assert_eq!(r_wide.width, 1920.0);
        assert_eq!(r_wide.x, (2560.0 - 1920.0) / 2.0); // 320.0
        assert_eq!(r_wide.y, 0.0);

        // 16:9 source in 1080x1920 vertical container (ratio < 16:9) -> letterbox
        let r_vert = ViewportRect::fit_aspect_ratio(16.0 / 9.0, 1080.0, 1920.0);
        assert_eq!(r_vert.width, 1080.0);
        let expected_h = 1080.0 / (16.0 / 9.0); // 607.5
        assert!((r_vert.height - expected_h).abs() < 1e-4);
        assert_eq!(r_vert.x, 0.0);
        assert!((r_vert.y - (1920.0 - expected_h) / 2.0).abs() < 1e-4);
    }
}
