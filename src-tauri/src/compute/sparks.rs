//! Particle Sparks GPU Compute Simulation Pipeline
//!
//! Simulates 65,536 particle sparks with stride 8 (pos.xy, vel.xy, age, life, hue, size),
//! exponential drag damping (exp(-dt * 1.2)), vertical gravity, and audio burst excitation.

use std::mem::size_of;

pub const MAX_SPARKS: usize = 65536;

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Particle {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub age: f32,
    pub life: f32,
    pub hue: f32,
    pub size: f32,
}

impl Default for Particle {
    fn default() -> Self {
        Self {
            position: [0.5, 0.5],
            velocity: [0.0, 0.0],
            age: 1.0,  // Dormant by default (age >= life)
            life: 1.0,
            hue: 0.0,
            size: 0.02,
        }
    }
}

#[repr(C, align(16))]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SparksUniforms {
    pub dt: f32,
    pub time: f32,
    pub aspect: f32,
    pub max_sparks: u32,
    pub emit_rate: f32,
    pub speed: f32,
    pub spread: f32,
    pub gravity: f32,
    pub life_param: f32,
    pub size_param: f32,
    pub hue_cycle: f32,
    pub emit_count: u32,
    pub mouse_pos: [f32; 2],
    pub mouse_vel: [f32; 2],
    pub mouse_active: u32,
    pub mouse_down: u32,
    pub emit_mode: u32,          // 0: stream, 1: on_move, 2: on_press
    pub audio_beat: f32,
    pub emit_start: u32,
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
}

impl Default for SparksUniforms {
    fn default() -> Self {
        Self {
            dt: 0.016,
            time: 0.0,
            aspect: 16.0 / 9.0,
            max_sparks: MAX_SPARKS as u32,
            emit_rate: 110.0,
            speed: 0.22,
            spread: 1.0,
            gravity: -0.06,
            life_param: 1.6,
            size_param: 0.02,
            hue_cycle: 0.1,
            emit_count: 0,
            mouse_pos: [0.5, 0.5],
            mouse_vel: [0.0, 0.0],
            mouse_active: 0,
            mouse_down: 0,
            emit_mode: 0,
            audio_beat: 0.0,
            emit_start: 0,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        }
    }
}

pub struct SparksComputePipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub uniform_buffer: wgpu::Buffer,
    pub storage_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub emit_cursor: u32,
}

impl SparksComputePipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        // Uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SparksUniformBuffer"),
            size: size_of::<SparksUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Storage buffer for 65,536 particles
        let storage_buffer_size = (MAX_SPARKS * size_of::<Particle>()) as u64;
        let storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SparksStorageBuffer"),
            size: storage_buffer_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SparksBindGroupLayout"),
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
            label: Some("SparksBindGroup"),
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

        // Compute shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SparksComputeShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/sparks_compute.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SparksComputePipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("SparksComputePipeline"),
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
            emit_cursor: 0,
        }
    }

    /// Reset all particles to dormant state.
    pub fn reset_particles(&self, queue: &wgpu::Queue) {
        let dormant = vec![Particle::default(); MAX_SPARKS];
        queue.write_buffer(&self.storage_buffer, 0, bytemuck::cast_slice(&dormant));
    }

    /// Advance emission cursor and return (start, count) tuple.
    pub fn advance_emission(&mut self, count: u32) -> (u32, u32) {
        let start = self.emit_cursor;
        self.emit_cursor = (self.emit_cursor + count) % (MAX_SPARKS as u32);
        (start, count)
    }

    /// Write particle slice directly (useful for tests and custom injectors).
    pub fn write_particles(&self, queue: &wgpu::Queue, offset: usize, particles: &[Particle]) {
        let byte_offset = (offset * size_of::<Particle>()) as u64;
        queue.write_buffer(&self.storage_buffer, byte_offset, bytemuck::cast_slice(particles));
    }

    /// Update uniforms for compute dispatch.
    pub fn update_uniforms(&self, queue: &wgpu::Queue, uniforms: &SparksUniforms) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    /// Dispatch compute pass.
    pub fn dispatch(&self, cpass: &mut wgpu::ComputePass) {
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &self.bind_group, &[]);
        let workgroups = (MAX_SPARKS as u32 + 63) / 64;
        cpass.dispatch_workgroups(workgroups, 1, 1);
    }

    /// Read back particle states synchronously for test verification.
    pub fn read_particles_sync(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        count: usize,
    ) -> Vec<Particle> {
        let count = count.min(MAX_SPARKS);
        let size = (count * size_of::<Particle>()) as u64;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SparksStagingBuffer"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SparksReadEncoder"),
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
        let result: Vec<Particle> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging.unmap();
        result
    }
}
