//! Floating Balls GPU Compute Simulation Pipeline
//!
//! Simulates up to 5,000 floating balls purely on the GPU with drift,
//! cruising speed damping, 4 mouse interaction modes, swell factor, and edge wrap/bounce.

use std::mem::size_of;

pub const MAX_BALLS: usize = 5000;

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Ball {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub angle: f32,
    pub phase: f32,
    pub speed_mult: f32,
    pub rand_val: f32,
    pub radius: f32,
    pub swell: f32,
    pub color_idx: u32,
    pub _pad: u32,
}

impl Default for Ball {
    fn default() -> Self {
        Self {
            pos: [0.5, 0.5],
            vel: [0.0, 0.0],
            angle: 0.0,
            phase: 0.0,
            speed_mult: 1.0,
            rand_val: 0.5,
            radius: 0.05,
            swell: 1.0,
            color_idx: 0,
            _pad: 0,
        }
    }
}

#[repr(C, align(16))]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BallsUniforms {
    pub dt: f32,
    pub time: f32,
    pub aspect: f32,
    pub count: u32,
    pub speed: f32,
    pub drift: f32,
    pub size: f32,
    pub size_var: f32,
    pub pulse: f32,
    pub swell_param: f32,
    pub softness: f32,
    pub alpha: f32,
    pub mouse_mode: u32,     // 0: off, 1: repel, 2: attract, 3: swirl, 4: stir
    pub mouse_active: u32,
    pub mouse_pos: [f32; 2],
    pub mouse_vel: [f32; 2],
    pub mouse_radius: f32,
    pub mouse_force: f32,
    pub edge_mode: u32,      // 0: wrap, 1: bounce
    pub audio_bass: f32,
    pub audio_beat: f32,
    pub hue_cycle: f32,
}

impl Default for BallsUniforms {
    fn default() -> Self {
        Self {
            dt: 0.016,
            time: 0.0,
            aspect: 16.0 / 9.0,
            count: 500,
            speed: 0.5,
            drift: 0.5,
            size: 0.08,
            size_var: 0.4,
            pulse: 0.15,
            swell_param: 0.8,
            softness: 0.75,
            alpha: 0.8,
            mouse_mode: 1, // Repel
            mouse_active: 0,
            mouse_pos: [0.5, 0.5],
            mouse_vel: [0.0, 0.0],
            mouse_radius: 0.25,
            mouse_force: 1.0,
            edge_mode: 0, // Wrap
            audio_bass: 0.0,
            audio_beat: 0.0,
            hue_cycle: 0.1,
        }
    }
}

pub struct BallsComputePipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub uniform_buffer: wgpu::Buffer,
    pub storage_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub count: u32,
}

impl BallsComputePipeline {
    pub fn new(device: &wgpu::Device, count: u32) -> Self {
        let count = count.min(MAX_BALLS as u32);

        // Uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BallsUniformBuffer"),
            size: size_of::<BallsUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Storage buffer for agents
        let storage_buffer_size = (MAX_BALLS * size_of::<Ball>()) as u64;
        let storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BallsStorageBuffer"),
            size: storage_buffer_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("BallsBindGroupLayout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BallsBindGroup"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: storage_buffer.as_entire_binding(),
                },
            ],
        });

        // Compute shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BallsComputeShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/balls_compute.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("BallsComputePipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("BallsComputePipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout,
            uniform_buffer,
            storage_buffer,
            bind_group,
            count,
        }
    }

    /// Seed the initial particle storage buffer with structured initial conditions.
    pub fn init_particles(&self, queue: &wgpu::Queue, count: u32, aspect: f32) {
        let n = count.min(MAX_BALLS as u32) as usize;
        let mut balls = Vec::with_capacity(n);

        for i in 0..n {
            let fi = i as f32;
            let fn_total = n as f32;
            let ratio = fi / fn_total.max(1.0);

            // Deterministic distribution across normalized aspect coordinates
            let px = (fi * 0.6180339887) % aspect;
            let py = ((fi * 1.41421356) % 1.0).clamp(0.05, 0.95);
            let angle = ratio * std::f32::consts::TAU;

            balls.push(Ball {
                pos: [px, py],
                vel: [0.0, 0.0],
                angle,
                phase: fi * 0.31,
                speed_mult: 0.6 + 0.8 * ((fi * 7.919) % 1.0),
                rand_val: (fi * 13.37) % 1.0,
                radius: 0.05,
                swell: 1.0,
                color_idx: (i % 8) as u32,
                _pad: 0,
            });
        }

        queue.write_buffer(&self.storage_buffer, 0, bytemuck::cast_slice(&balls));
    }

    /// Update uniforms for compute dispatch.
    pub fn update_uniforms(&self, queue: &wgpu::Queue, uniforms: &BallsUniforms) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    /// Dispatch compute pass.
    pub fn dispatch(&self, cpass: &mut wgpu::ComputePass) {
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &self.bind_group, &[]);
        let workgroups = (self.count + 63) / 64;
        cpass.dispatch_workgroups(workgroups, 1, 1);
    }

    /// Read back particle states synchronously for unit testing and state verification.
    pub fn read_balls_sync(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<Ball> {
        let size = (self.count as usize * size_of::<Ball>()) as u64;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BallsStagingBuffer"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("BallsReadEncoder"),
        });
        encoder.copy_buffer_to_buffer(&self.storage_buffer, 0, &staging, 0, size);
        queue.submit(std::iter::once(encoder.finish()));

        let slice = staging.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = sender.send(res);
        });
        device.poll(wgpu::Maintain::Wait);
        receiver
            .recv()
            .expect("Failed to receive buffer map completion")
            .expect("Buffer mapping failed");

        let data = slice.get_mapped_range();
        let result: Vec<Ball> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging.unmap();
        result
    }
}
