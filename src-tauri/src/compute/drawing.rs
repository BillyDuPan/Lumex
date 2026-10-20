//! GPU Vector Brush Drawing Pipeline
//!
//! Renders instanced line segments with analytical Signed Distance Fields (SDFs),
//! sub-pixel antialiasing, multi-pass neon exponential glow, round caps, round joins,
//! vanishing stroke decay, and persistent canvas accumulation textures with eraser support.

use std::mem::size_of;

pub const MAX_SEGMENTS: usize = 16384;

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BrushStyle {
    Solid = 0,
    Neon = 1,
    Soft = 2,
    Erase = 3,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StrokeSegment {
    pub p0: [f32; 2],
    pub p1: [f32; 2],
    pub size: f32,
    pub birth_time: f32,
    pub lifetime: f32,
    pub style: u32,
    pub color: [f32; 4],
}

impl Default for StrokeSegment {
    fn default() -> Self {
        Self {
            p0: [0.0, 0.0],
            p1: [0.0, 0.0],
            size: 14.0,
            birth_time: 0.0,
            lifetime: 0.0, // Permanent
            style: BrushStyle::Solid as u32,
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

#[repr(C, align(16))]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawUniforms {
    pub viewport_size: [f32; 2],
    pub time: f32,
    pub aspect: f32,
    pub is_eraser: u32,
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
}

impl Default for DrawUniforms {
    fn default() -> Self {
        Self {
            viewport_size: [1920.0, 1080.0],
            time: 0.0,
            aspect: 16.0 / 9.0,
            is_eraser: 0,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        }
    }
}

pub struct DrawingPipeline {
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub uniform_buffer: wgpu::Buffer,
    pub segment_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub normal_pipeline: wgpu::RenderPipeline,
    pub eraser_pipeline: wgpu::RenderPipeline,
    pub target_format: wgpu::TextureFormat,

    // Persistent canvas accumulation texture
    pub canvas_texture: wgpu::Texture,
    pub canvas_view: wgpu::TextureView,
    pub canvas_width: u32,
    pub canvas_height: u32,
}

impl DrawingPipeline {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let width = width.max(1);
        let height = height.max(1);

        // Uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DrawUniformBuffer"),
            size: size_of::<DrawUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Segment storage buffer
        let segment_buffer_size = (MAX_SEGMENTS * size_of::<StrokeSegment>()) as u64;
        let segment_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("StrokeSegmentStorageBuffer"),
            size: segment_buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DrawBindGroupLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DrawBindGroup"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: segment_buffer.as_entire_binding(),
                },
            ],
        });

        // Shaders
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BrushSdfShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/brush_sdf.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("BrushPipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Normal blend pipeline (Alpha blending)
        let normal_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BrushNormalPipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Eraser blend pipeline (destination-out)
        let eraser_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("BrushEraserPipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create persistent accumulation texture
        let canvas_desc = wgpu::TextureDescriptor {
            label: Some("DrawingCanvasAccumulationTexture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };
        let canvas_texture = device.create_texture(&canvas_desc);
        let canvas_view = canvas_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            bind_group_layout,
            uniform_buffer,
            segment_buffer,
            bind_group,
            normal_pipeline,
            eraser_pipeline,
            target_format,
            canvas_texture,
            canvas_view,
            canvas_width: width,
            canvas_height: height,
        }
    }

    /// Resize canvas accumulation texture.
    pub fn resize_canvas(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == 0 || height == 0 || (width == self.canvas_width && height == self.canvas_height) {
            return;
        }

        self.canvas_width = width;
        self.canvas_height = height;

        let canvas_desc = wgpu::TextureDescriptor {
            label: Some("DrawingCanvasAccumulationTexture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };
        self.canvas_texture = device.create_texture(&canvas_desc);
        self.canvas_view = self.canvas_texture.create_view(&wgpu::TextureViewDescriptor::default());
    }

    /// Clear canvas accumulation texture with transparent black.
    pub fn clear_canvas(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("ClearCanvasEncoder"),
        });
        {
            let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ClearCanvasPass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.canvas_view,
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
        }
        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Upload stroke segments into storage buffer.
    pub fn upload_segments(&self, queue: &wgpu::Queue, segments: &[StrokeSegment]) {
        let count = segments.len().min(MAX_SEGMENTS);
        if count > 0 {
            queue.write_buffer(
                &self.segment_buffer,
                0,
                bytemuck::cast_slice(&segments[..count]),
            );
        }
    }

    /// Update draw uniforms.
    pub fn update_uniforms(&self, queue: &wgpu::Queue, uniforms: &DrawUniforms) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    /// Render instanced segments to an active render pass.
    pub fn render_segments(
        &self,
        rpass: &mut wgpu::RenderPass,
        segment_count: u32,
        is_eraser: bool,
    ) {
        let count = segment_count.min(MAX_SEGMENTS as u32);
        if count == 0 {
            return;
        }

        if is_eraser {
            rpass.set_pipeline(&self.eraser_pipeline);
        } else {
            rpass.set_pipeline(&self.normal_pipeline);
        }

        rpass.set_bind_group(0, &self.bind_group, &[]);
        // Each segment is rendered as a 6-vertex quad (2 triangles)
        rpass.draw(0..6, 0..count);
    }
}
