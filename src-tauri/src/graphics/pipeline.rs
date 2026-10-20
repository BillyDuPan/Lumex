//! Lumex Multi-Pass Post-Processing Pipeline
//!
//! Executes the 4-pass GPU post-processing pipeline:
//! 1. Feedback Echo Accumulation (ping-pong Rgba16Float)
//! 2. 13-Tap Blur Pyramid Downsampling (4 mip levels)
//! 3. Tent Filter Upsampling & Bloom Glow Mix
//! 4. Final Composite (Warp, Kaleidoscope, Infinite Zoom, RGB Split, Color Grading, Dither)

use std::collections::HashMap;
use std::sync::Arc;

use crate::graphics::targets::{PostProcessTargets, BLUR_LEVELS};
use crate::graphics::uniforms::FxUniforms;

#[repr(C, align(16))]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DownsampleUniforms {
    pub texel_size: [f32; 2],
    pub _pad: [f32; 2],
}

#[repr(C, align(16))]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CompUniforms {
    pub blur_glow: [f32; 4],
    pub t2: [f32; 2],
    pub t3: [f32; 2],
    pub t4: [f32; 2],
    pub _pad: [f32; 2],
}

pub struct PostProcessPipeline {
    pub sampler: wgpu::Sampler,

    // Uniform buffers
    pub fx_buffer: wgpu::Buffer,
    pub downsample_buffers: Vec<wgpu::Buffer>,
    pub comp_buffer: wgpu::Buffer,

    // Bind group layouts
    pub echo_layout: wgpu::BindGroupLayout,
    pub downsample_layout: wgpu::BindGroupLayout,
    pub comp_layout: wgpu::BindGroupLayout,
    pub final_layout: wgpu::BindGroupLayout,

    // Pipeline layouts
    pub echo_pipeline: wgpu::RenderPipeline,
    pub downsample_pipeline: wgpu::RenderPipeline,
    pub comp_pipeline: wgpu::RenderPipeline,

    // Final composite pipelines cached per target texture format
    pub final_pipeline_layout: wgpu::PipelineLayout,
    pub final_shader_module: Arc<wgpu::ShaderModule>,
    pub final_pipelines: HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>,
}

impl PostProcessPipeline {
    pub fn new(device: &wgpu::Device, default_output_format: wgpu::TextureFormat) -> Self {
        // Linear sampler with mirrored repeat to eliminate feedback border seams
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("PostProcessSampler"),
            address_mode_u: wgpu::AddressMode::MirrorRepeat,
            address_mode_v: wgpu::AddressMode::MirrorRepeat,
            address_mode_w: wgpu::AddressMode::MirrorRepeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Buffers
        let fx_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("FxUniformBuffer"),
            size: std::mem::size_of::<FxUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut downsample_buffers = Vec::with_capacity(BLUR_LEVELS);
        for i in 0..BLUR_LEVELS {
            downsample_buffers.push(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("DownsampleUniformBuffer{}", i)),
                size: std::mem::size_of::<DownsampleUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }

        let comp_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("CompUniformBuffer"),
            size: std::mem::size_of::<CompUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Shader modules
        let echo_sm = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("EchoShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/echo.wgsl").into()),
        });

        let downsample_sm = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DownsampleShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/downsample13.wgsl").into()),
        });

        let comp_sm = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("CompShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/blur_glow_comp.wgsl").into()),
        });

        let final_sm = Arc::new(device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("FinalCompositeShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/final_composite.wgsl").into()),
        }));

        // 1. Echo Bind Group Layout
        let echo_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("EchoBindGroupLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
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

        // 2. Downsample Bind Group Layout
        let downsample_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DownsampleBindGroupLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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

        // 3. Blur/Glow Composite Bind Group Layout
        let comp_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("CompBindGroupLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
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

        // 4. Final Composite Bind Group Layout
        let final_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("FinalBindGroupLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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

        // Render Pipelines
        let echo_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("EchoPipelineLayout"),
            bind_group_layouts: &[&echo_layout],
            push_constant_ranges: &[],
        });

        let echo_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("EchoPipeline"),
            layout: Some(&echo_pl),
            vertex: wgpu::VertexState {
                module: &echo_sm,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &echo_sm,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let downsample_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DownsamplePipelineLayout"),
            bind_group_layouts: &[&downsample_layout],
            push_constant_ranges: &[],
        });

        let downsample_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("DownsamplePipeline"),
            layout: Some(&downsample_pl),
            vertex: wgpu::VertexState {
                module: &downsample_sm,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &downsample_sm,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let comp_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("CompPipelineLayout"),
            bind_group_layouts: &[&comp_layout],
            push_constant_ranges: &[],
        });

        let comp_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("CompPipeline"),
            layout: Some(&comp_pl),
            vertex: wgpu::VertexState {
                module: &comp_sm,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &comp_sm,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let final_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("FinalPipelineLayout"),
            bind_group_layouts: &[&final_layout],
            push_constant_ranges: &[],
        });

        let mut final_pipelines = HashMap::new();
        let default_final = Self::build_final_pipeline(
            device,
            &final_pipeline_layout,
            &final_sm,
            default_output_format,
        );
        final_pipelines.insert(default_output_format, default_final);

        Self {
            sampler,
            fx_buffer,
            downsample_buffers,
            comp_buffer,
            echo_layout,
            downsample_layout,
            comp_layout,
            final_layout,
            echo_pipeline,
            downsample_pipeline,
            comp_pipeline,
            final_pipeline_layout,
            final_shader_module: final_sm,
            final_pipelines,
        }
    }

    fn build_final_pipeline(
        device: &wgpu::Device,
        layout: &wgpu::PipelineLayout,
        shader: &wgpu::ShaderModule,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("FinalCompositePipeline_{:?}", format)),
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
                    blend: None,
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

    pub fn get_or_create_final_pipeline(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> &wgpu::RenderPipeline {
        self.final_pipelines.entry(format).or_insert_with(|| {
            Self::build_final_pipeline(
                device,
                &self.final_pipeline_layout,
                &self.final_shader_module,
                format,
            )
        })
    }

    /// Execute the complete multi-pass post-processing pipeline.
    ///
    /// Pipeline sequence:
    /// 1. Feedback Echo Accumulation (Pass 1) -> ping-pong Rgba16Float
    /// 2. Blur Pyramid Downsampling (Pass 2) -> 4 levels
    /// 3. Blur/Glow Composite (Pass 3) -> comp_target
    /// 4. Final Composite (Pass 4) -> output_view
    pub fn process(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        targets: &mut PostProcessTargets,
        uniforms: &FxUniforms,
        output_view: &wgpu::TextureView,
        output_format: wgpu::TextureFormat,
    ) {
        // Upload FX uniforms
        queue.write_buffer(&self.fx_buffer, 0, bytemuck::bytes_of(uniforms));

        // Determine base source texture
        let mut base_view = &targets.src_target.view;

        // -------------------------------------------------------------
        // PASS 1: Echo / Feedback Accumulation
        // -------------------------------------------------------------
        let echo_amt = uniforms.feedback[0];
        if echo_amt > 0.001 {
            let next_target = targets.echo_targets.next();
            let next_view = &next_target.view;
            let prev_view = &targets.echo_targets.current().view;

            let echo_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("EchoBindGroup"),
                layout: &self.echo_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.fx_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&targets.src_target.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(prev_view),
                    },
                ],
            });

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Pass1_Echo"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: next_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                pass.set_pipeline(&self.echo_pipeline);
                pass.set_bind_group(0, &echo_bg, &[]);
                pass.draw(0..3, 0..1);
            }

            targets.echo_targets.swap();
            base_view = &targets.echo_targets.current().view;
        }

        // -------------------------------------------------------------
        // PASS 2 & 3: Blur Pyramid & Tent Glow Composite
        // -------------------------------------------------------------
        let blur_amt = uniforms.blur_glow[0];
        let glow_amt = uniforms.blur_glow[1];
        let needs_blur_glow = blur_amt > 0.001 || glow_amt > 0.001;

        let mut final_source_view = base_view;

        if needs_blur_glow {
            // Downsampling passes across 4 pyramid levels
            let mut from_view = base_view;
            let mut from_w = targets.width;
            let mut from_h = targets.height;

            for i in 0..BLUR_LEVELS {
                let to_level = &targets.pyramid.levels[i];
                let texel_uniforms = DownsampleUniforms {
                    texel_size: [1.0 / from_w as f32, 1.0 / from_h as f32],
                    _pad: [0.0, 0.0],
                };
                queue.write_buffer(
                    &self.downsample_buffers[i],
                    0,
                    bytemuck::bytes_of(&texel_uniforms),
                );

                let down_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("DownsampleBindGroupLevel{}", i + 1)),
                    layout: &self.downsample_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.downsample_buffers[i].as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(from_view),
                        },
                    ],
                });

                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some(&format!("Pass2_Downsample_L{}", i + 1)),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &to_level.view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    pass.set_pipeline(&self.downsample_pipeline);
                    pass.set_bind_group(0, &down_bg, &[]);
                    pass.draw(0..3, 0..1);
                }

                from_view = &to_level.view;
                from_w = to_level.width;
                from_h = to_level.height;
            }

            // Composite blur & glow
            let comp_uniforms = CompUniforms {
                blur_glow: uniforms.blur_glow,
                t2: targets.pyramid.levels[1].texel_size(),
                t3: targets.pyramid.levels[2].texel_size(),
                t4: targets.pyramid.levels[3].texel_size(),
                _pad: [0.0, 0.0],
            };
            queue.write_buffer(&self.comp_buffer, 0, bytemuck::bytes_of(&comp_uniforms));

            let comp_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("CompBindGroup"),
                layout: &self.comp_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.comp_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(base_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&targets.pyramid.levels[0].view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&targets.pyramid.levels[1].view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::TextureView(&targets.pyramid.levels[2].view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::TextureView(&targets.pyramid.levels[3].view),
                    },
                ],
            });

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Pass3_BlurGlowComp"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &targets.comp_target.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                pass.set_pipeline(&self.comp_pipeline);
                pass.set_bind_group(0, &comp_bg, &[]);
                pass.draw(0..3, 0..1);
            }

            final_source_view = &targets.comp_target.view;
        }

        // -------------------------------------------------------------
        // PASS 4: Final Warp / Kaleidoscope / Zoom / Color Grading Pass
        // -------------------------------------------------------------
        let final_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("FinalBindGroup"),
            layout: &self.final_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.fx_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(final_source_view),
                },
            ],
        });

        let final_pipe = self.get_or_create_final_pipeline(device, output_format);

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Pass4_FinalComposite"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(final_pipe);
            pass.set_bind_group(0, &final_bg, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
