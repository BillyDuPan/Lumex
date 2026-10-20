//! Craig Reynolds Intelligent Life Boids GPU Compute Pipeline
//!
//! Simulates up to 10,000 schooling boid agents in parallel with toroidal wrapping,
//! separation (R_sep = 0.38 R_sense), alignment, cohesion, food spore foraging,
//! and predator mouse interactions (hunt, flee, orbit).

use std::mem::size_of;

pub const MAX_BOIDS: usize = 10000;
pub const MAX_SPORES: usize = 64;

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Boid {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub target_pos: [f32; 2],
    pub light: f32,
    pub energy: f32,
    pub species: u32,
    pub phase: f32,
    pub _pad: [f32; 2],
}

impl Default for Boid {
    fn default() -> Self {
        Self {
            pos: [0.5, 0.5],
            vel: [0.05, 0.0],
            target_pos: [0.0, 0.0],
            light: 0.15,
            energy: 1.0,
            species: 0,
            phase: 0.0,
            _pad: [0.0, 0.0],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Spore {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub pulse: f32,
    pub _pad: [f32; 3],
}

impl Default for Spore {
    fn default() -> Self {
        Self {
            pos: [0.5, 0.5],
            vel: [0.0, 0.0],
            pulse: 1.0,
            _pad: [0.0; 3],
        }
    }
}

#[repr(C, align(16))]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BoidsUniforms {
    pub dt: f32,
    pub time: f32,
    pub aspect: f32,
    pub boid_count: u32,
    pub spore_count: u32,
    pub speed: f32,
    pub flock: f32,
    pub separation: f32,
    pub sense: f32,
    pub biolum: f32,
    pub mouse_mode: u32,         // 0: hunt, 1: flee, 2: orbit, 3: off
    pub mouse_active: u32,
    pub mouse_pos: [f32; 2],
    pub mouse_radius: f32,
    pub mouse_force: f32,
    pub edge_mode: u32,          // 0: wrap, 1: bounce
    pub life_type: u32,          // 0: swimmer, 1: microbe, 2: spore
    pub audio_beat: f32,
    pub max_neighbors: u32,
}

impl Default for BoidsUniforms {
    fn default() -> Self {
        Self {
            dt: 0.016,
            time: 0.0,
            aspect: 16.0 / 9.0,
            boid_count: 500,
            spore_count: 6,
            speed: 0.7,
            flock: 0.9,
            separation: 1.1,
            sense: 0.28,
            biolum: 1.0,
            mouse_mode: 0, // Hunt
            mouse_active: 0,
            mouse_pos: [0.5, 0.5],
            mouse_radius: 0.25,
            mouse_force: 1.0,
            edge_mode: 0, // Wrap
            life_type: 0, // Swimmer
            audio_beat: 0.0,
            max_neighbors: 128,
        }
    }
}

pub struct BoidsComputePipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub uniform_buffer: wgpu::Buffer,
    pub boids_buffer: wgpu::Buffer,
    pub spores_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub boid_count: u32,
    pub spore_count: u32,
}

impl BoidsComputePipeline {
    pub fn new(device: &wgpu::Device, boid_count: u32, spore_count: u32) -> Self {
        let boid_count = boid_count.min(MAX_BOIDS as u32);
        let spore_count = spore_count.min(MAX_SPORES as u32);

        // Uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BoidsUniformBuffer"),
            size: size_of::<BoidsUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Boids storage buffer
        let boids_buffer_size = (MAX_BOIDS * size_of::<Boid>()) as u64;
        let boids_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BoidsStorageBuffer"),
            size: boids_buffer_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Spores storage buffer
        let spores_buffer_size = (MAX_SPORES * size_of::<Spore>()) as u64;
        let spores_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SporesStorageBuffer"),
            size: spores_buffer_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("BoidsBindGroupLayout"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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
            label: Some("BoidsBindGroup"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: boids_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: spores_buffer.as_entire_binding(),
                },
            ],
        });

        // Shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("BoidsComputeShader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/boids_compute.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("BoidsComputePipelineLayout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("BoidsComputePipeline"),
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
            boids_buffer,
            spores_buffer,
            bind_group,
            boid_count,
            spore_count,
        }
    }

    /// Initialize boids and spores across the simulation viewport.
    pub fn init_boids_and_spores(&self, queue: &wgpu::Queue, aspect: f32) {
        let count = self.boid_count as usize;
        let mut boids = Vec::with_capacity(count);

        for i in 0..count {
            let fi = i as f32;
            let px = (fi * 0.6180339887) % aspect;
            let py = ((fi * 1.41421356) % 1.0).clamp(0.05, 0.95);
            let angle = ((fi * 2.71828) % 1.0) * std::f32::consts::TAU;

            let spd = 0.12;
            boids.push(Boid {
                pos: [px, py],
                vel: [angle.cos() * spd, angle.sin() * spd],
                target_pos: [0.0, 0.0],
                light: 0.15,
                energy: 0.8 + 0.4 * ((fi * 3.14159) % 1.0),
                species: (i % 3) as u32,
                phase: fi * 0.47,
                _pad: [0.0, 0.0],
            });
        }
        queue.write_buffer(&self.boids_buffer, 0, bytemuck::cast_slice(&boids));

        let scount = self.spore_count as usize;
        let mut spores = Vec::with_capacity(scount);
        for s in 0..scount {
            let fs = s as f32;
            spores.push(Spore {
                pos: [(fs * 0.732) % aspect, (fs * 0.523) % 1.0],
                vel: [0.0, 0.0],
                pulse: 1.0,
                _pad: [0.0; 3],
            });
        }
        queue.write_buffer(&self.spores_buffer, 0, bytemuck::cast_slice(&spores));
    }

    /// Write custom boid data directly (for tests).
    pub fn write_boids(&self, queue: &wgpu::Queue, boids: &[Boid]) {
        queue.write_buffer(&self.boids_buffer, 0, bytemuck::cast_slice(boids));
    }

    /// Write custom spore data directly (for tests).
    pub fn write_spores(&self, queue: &wgpu::Queue, spores: &[Spore]) {
        queue.write_buffer(&self.spores_buffer, 0, bytemuck::cast_slice(spores));
    }

    /// Update uniforms for compute dispatch.
    pub fn update_uniforms(&self, queue: &wgpu::Queue, uniforms: &BoidsUniforms) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    /// Dispatch compute pass.
    pub fn dispatch(&self, cpass: &mut wgpu::ComputePass) {
        cpass.set_pipeline(&self.pipeline);
        cpass.set_bind_group(0, &self.bind_group, &[]);
        let workgroups = (self.boid_count + 63) / 64;
        cpass.dispatch_workgroups(workgroups, 1, 1);
    }

    /// Read back boid states synchronously for test verification.
    pub fn read_boids_sync(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        count: usize,
    ) -> Vec<Boid> {
        let count = count.min(MAX_BOIDS);
        let size = (count * size_of::<Boid>()) as u64;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BoidsStagingBuffer"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("BoidsReadEncoder"),
        });
        encoder.copy_buffer_to_buffer(&self.boids_buffer, 0, &staging, 0, size);
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
        let result: Vec<Boid> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging.unmap();
        result
    }

    /// Read back spore states synchronously.
    pub fn read_spores_sync(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        count: usize,
    ) -> Vec<Spore> {
        let count = count.min(MAX_SPORES);
        let size = (count * size_of::<Spore>()) as u64;
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SporesStagingBuffer"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("SporesReadEncoder"),
        });
        encoder.copy_buffer_to_buffer(&self.spores_buffer, 0, &staging, 0, size);
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
        let result: Vec<Spore> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging.unmap();
        result
    }
}
