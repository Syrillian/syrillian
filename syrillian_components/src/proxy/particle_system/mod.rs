// TODO: refactor

use crate::particle_system::ParticleSystemSettings;
use bytemuck::bytes_of;
use std::any::Any;
use std::mem::size_of;
use std::time::Instant;
use syrillian::assets::defaults::ParticleVertex;
use syrillian::assets::{HComputeShader, HShader};
use syrillian::math::{Affine3A, Vec3};
use syrillian::syrillian_macros::UniformIndex;
use syrillian::wgpu::{
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor,
};
use syrillian_render::proxies::{
    PROXY_PRIORITY_SOLID, PROXY_PRIORITY_TRANSPARENT, SceneProxy, SceneProxyBinding,
};
use syrillian_render::rendering::GPUDrawCtx;
use syrillian_render::rendering::renderer::Renderer;
use syrillian_render::rendering::uniform::ShaderUniform;
use syrillian_render::{AssetCache, proxy_data, proxy_data_mut};
use syrillian_utils::{ShaderUniformIndex, ShaderUniformMultiIndex};

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum ParticleUniformIndex {
    Settings = 0,
    Runtime = 1,
}

impl ShaderUniformIndex for ParticleUniformIndex {
    const MAX: usize = 1;

    fn index(&self) -> usize {
        *self as usize
    }

    fn by_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Settings),
            1 => Some(Self::Runtime),
            _ => None,
        }
    }

    fn name() -> &'static str {
        "Particle"
    }
}

impl ShaderUniformMultiIndex for ParticleUniformIndex {}

#[repr(u8)]
#[derive(Copy, Clone, Debug, UniformIndex)]
pub enum ParticleComputeUniformIndex {
    Settings = 0,
    Runtime = 1,
    Output = 2,
    Dispatch = 3,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleSystemUniform {
    pub position: [f32; 4],
    pub velocity: [f32; 4],
    pub acceleration: [f32; 4],
    pub color: [f32; 4],
    pub end_color: [f32; 4],
    // x=opacity, y=end_opacity, z=lifetime, w=duration
    pub emitter: [f32; 4],
    // x=spawn_rate, y=turbulence_strength, z=turbulence_scale, w=turbulence_speed
    pub emission: [f32; 4],
    // x=min, y=max
    pub lifetime_random: [f32; 4],
    // x=seed, y=particle_count, z=burst_count, w=looping
    pub counts: [u32; 4],
    pub position_random_min: [f32; 4],
    pub position_random_max: [f32; 4],
    pub velocity_random_min: [f32; 4],
    pub velocity_random_max: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleRuntimeUniform {
    // x=elapsed_time
    pub data: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ParticleDispatchUniform {
    pub start_index: u32,
    pub chunk_count: u32,
    pub total_count: u32,
    pub _pad0: u32,
}

#[derive(Debug)]
struct ParticleChunkGpu {
    compute_uniform: ShaderUniform<ParticleComputeUniformIndex>,
    particle_buffer: syrillian::wgpu::Buffer,
    count: u32,
}

#[derive(Debug)]
pub struct ParticleSystemGpuData {
    render_uniform: ShaderUniform<ParticleUniformIndex>,
    chunks: Vec<ParticleChunkGpu>,
    runtime: ParticleRuntimeUniform,
}

#[derive(Debug)]
pub struct ParticleSystemProxy {
    pub settings: ParticleSystemSettings,
    pub particle_count: u32,
    pub start_time: Instant,
}

impl ParticleSystemUniform {
    pub fn new(settings: &ParticleSystemSettings, particle_count: u32) -> Self {
        let vec3 = |v: Vec3| [v.x, v.y, v.z, 0.0];
        Self {
            position: vec3(settings.position),
            velocity: vec3(settings.velocity),
            acceleration: vec3(settings.acceleration),
            color: vec3(settings.color),
            end_color: vec3(settings.end_color),
            emitter: [
                settings.opacity,
                settings.end_opacity,
                settings.lifetime,
                settings.duration,
            ],
            emission: [
                settings.spawn_rate,
                settings.turbulence_strength,
                settings.turbulence_scale,
                settings.turbulence_speed,
            ],
            lifetime_random: [
                settings.lifetime_random_min,
                settings.lifetime_random_max,
                0.0,
                0.0,
            ],
            counts: [
                settings.seed,
                particle_count,
                settings.start_count,
                settings.looping as u32,
            ],
            position_random_min: vec3(settings.position_random_min),
            position_random_max: vec3(settings.position_random_max),
            velocity_random_min: vec3(settings.velocity_random_min),
            velocity_random_max: vec3(settings.velocity_random_max),
        }
    }
}

impl Default for ParticleRuntimeUniform {
    fn default() -> Self {
        Self::const_default()
    }
}

impl ParticleRuntimeUniform {
    pub const fn const_default() -> Self {
        Self { data: [0.0; 4] }
    }
}

impl SceneProxy for ParticleSystemProxy {
    fn setup_render(
        &mut self,
        renderer: &Renderer,
        _local_to_world: &Affine3A,
    ) -> Box<dyn Any + Send> {
        let settings = ParticleSystemUniform::new(&self.settings, self.particle_count);
        let runtime = ParticleRuntimeUniform::const_default();

        let render_uniform =
            ShaderUniform::<ParticleUniformIndex>::builder(renderer.cache.bgl_particle_render())
                .with_buffer_data(&settings)
                .with_buffer_data(&runtime)
                .build(&renderer.state.device);
        let limits = renderer.state.device.limits();
        let max_storage_binding = limits.max_storage_buffer_binding_size as u64;
        let max_buffer_size = limits.max_buffer_size;
        let chunk_byte_limit = max_storage_binding.min(max_buffer_size).max(4);
        let max_particles_by_buffer =
            (chunk_byte_limit / size_of::<ParticleVertex>() as u64).max(1) as u32;
        let max_particles_by_dispatch = limits
            .max_compute_workgroups_per_dimension
            .saturating_mul(64)
            .max(1);
        let max_particles_per_chunk = max_particles_by_buffer.min(max_particles_by_dispatch);

        let mut chunks = Vec::new();
        let mut start = 0u32;
        while start < self.particle_count {
            let count = (self.particle_count - start).min(max_particles_per_chunk);
            let particle_buffer_size = ((count as u64) * size_of::<ParticleVertex>() as u64).max(4);
            let particle_buffer = renderer.state.device.create_buffer(&BufferDescriptor {
                label: Some("Particle Compute Vertex Buffer"),
                size: particle_buffer_size,
                usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let dispatch = ParticleDispatchUniform {
                start_index: start,
                chunk_count: count,
                total_count: self.particle_count,
                _pad0: 0,
            };

            let compute_uniform = ShaderUniform::<ParticleComputeUniformIndex>::builder(
                renderer.cache.bgl_particle_compute(),
            )
            .with_buffer(
                render_uniform
                    .buffer(ParticleUniformIndex::Settings)
                    .clone(),
            )
            .with_buffer(render_uniform.buffer(ParticleUniformIndex::Runtime).clone())
            .with_storage_buffer(particle_buffer.clone())
            .with_buffer_data(&dispatch)
            .build(&renderer.state.device);

            chunks.push(ParticleChunkGpu {
                compute_uniform,
                particle_buffer,
                count,
            });
            start += count;
        }

        self.start_time = Instant::now();

        Box::new(ParticleSystemGpuData {
            render_uniform,
            chunks,
            runtime,
        })
    }

    fn refresh_transform(
        &mut self,
        _renderer: &Renderer,
        _data: &mut (dyn Any + Send),
        _local_to_world: &Affine3A,
    ) {
    }

    fn update_render(
        &mut self,
        renderer: &Renderer,
        data: &mut (dyn Any + Send),
        _local_to_world: &Affine3A,
    ) {
        let data: &mut ParticleSystemGpuData = proxy_data_mut!(data);
        let settings = ParticleSystemUniform::new(&self.settings, self.particle_count);
        renderer.state.queue.write_buffer(
            data.render_uniform.buffer(ParticleUniformIndex::Settings),
            0,
            bytes_of(&settings),
        );

        data.runtime.data[0] = self.start_time.elapsed().as_secs_f32();
        renderer.state.queue.write_buffer(
            data.render_uniform.buffer(ParticleUniformIndex::Runtime),
            0,
            bytes_of(&data.runtime),
        );

        let mut encoder = renderer
            .state
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Particle Compute Encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Particle Position Compute Pass"),
                ..ComputePassDescriptor::default()
            });

            let shader = renderer
                .cache
                .compute_shader(HComputeShader::PARTICLE_POSITION);
            pass.set_pipeline(shader.pipeline());
            for chunk in &data.chunks {
                if chunk.count == 0 {
                    continue;
                }
                pass.set_bind_group(0, chunk.compute_uniform.bind_group(), &[]);
                pass.dispatch_workgroups(chunk.count.div_ceil(64), 1, 1);
            }
        }

        renderer.state.queue.submit(Some(encoder.finish()));
    }

    fn render(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        let transparent = self.settings.opacity < 1.0 || self.settings.end_opacity < 1.0;
        if transparent ^ ctx.transparency_pass {
            return;
        }

        let data: &ParticleSystemGpuData = proxy_data!(binding.proxy_data());
        let shader = renderer.cache.shader(HShader::PARTICLE_SYSTEM);
        let mut pass = ctx.pass.write();

        if !shader.activate(&mut pass, ctx) {
            return;
        }
        if let Some(idx) = shader.bind_groups().model {
            pass.set_bind_group(idx, data.render_uniform.bind_group(), &[]);
        }

        for chunk in &data.chunks {
            if chunk.count == 0 {
                continue;
            }
            pass.set_vertex_buffer(0, chunk.particle_buffer.slice(..));
            pass.draw(0..chunk.count, 0..1);
        }
    }

    fn priority(&self, _cache: Option<&AssetCache>) -> u32 {
        if self.settings.opacity < 1.0 || self.settings.end_opacity < 1.0 {
            PROXY_PRIORITY_TRANSPARENT
        } else {
            PROXY_PRIORITY_SOLID
        }
    }
}
