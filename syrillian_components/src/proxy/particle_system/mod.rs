use crate::particle_system::ParticleSystemSettings;
use bytemuck::bytes_of;
use std::any::Any;
use std::time::Instant;
use syrillian::assets::defaults::DEFAULT_COLOR_TARGETS;
use syrillian::assets::store::StoreType;
use syrillian::assets::{AssetStore, HShader, Shader, ShaderCode, ShaderType};
use syrillian::math::{Affine3A, Vec3};
use syrillian::wgpu::PrimitiveTopology;
use syrillian_render::proxies::{
    PROXY_PRIORITY_SOLID, PROXY_PRIORITY_TRANSPARENT, SceneProxy, SceneProxyBinding,
};
use syrillian_render::rendering::GPUDrawCtx;
use syrillian_render::rendering::renderer::Renderer;
use syrillian_render::rendering::uniform::ShaderUniform;
use syrillian_render::{proxy_data, proxy_data_mut};
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
        "Particle Uniform"
    }
}

impl ShaderUniformMultiIndex for ParticleUniformIndex {}

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

#[derive(Debug)]
pub struct ParticleSystemGpuData {
    shader: HShader,
    uniform: ShaderUniform<ParticleUniformIndex>,
    runtime: ParticleRuntimeUniform,
}

#[derive(Debug)]
pub struct ParticleSystemProxy {
    pub settings: ParticleSystemSettings,
    pub particle_count: u32,
    pub start_time: Instant,
}

impl ParticleSystemUniform {
    pub fn new(settings: ParticleSystemSettings, particle_count: u32) -> Self {
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
        let store = renderer.cache.store();
        let shader = Shader::builder()
            .name("Particle System")
            .color_target(DEFAULT_COLOR_TARGETS)
            .shader_type(ShaderType::Custom)
            .vertex_buffers(&[])
            .topology(PrimitiveTopology::PointList)
            .code(ShaderCode::Full(
                include_str!("particle_system_shader.wgsl").to_string(),
            ))
            .build()
            .store(store);

        let settings = ParticleSystemUniform::new(self.settings, self.particle_count);
        let runtime = ParticleRuntimeUniform::const_default();
        let uniform = ShaderUniform::<ParticleUniformIndex>::builder(renderer.cache.bgl_model())
            .with_buffer_data(&settings)
            .with_buffer_data(&runtime)
            .build(&renderer.state.device);

        self.start_time = Instant::now();

        Box::new(ParticleSystemGpuData {
            shader,
            uniform,
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
        let settings = ParticleSystemUniform::new(self.settings, self.particle_count);
        renderer.state.queue.write_buffer(
            data.uniform.buffer(ParticleUniformIndex::Settings),
            0,
            bytes_of(&settings),
        );

        data.runtime.data[0] = self.start_time.elapsed().as_secs_f32();
        renderer.state.queue.write_buffer(
            data.uniform.buffer(ParticleUniformIndex::Runtime),
            0,
            bytes_of(&data.runtime),
        );
    }

    fn render(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        let transparent = self.settings.opacity < 1.0 || self.settings.end_opacity < 1.0;
        if transparent ^ ctx.transparency_pass {
            return;
        }

        let data: &ParticleSystemGpuData = proxy_data!(binding.proxy_data());
        let shader = renderer.cache.shader(data.shader);
        let mut pass = ctx.pass.write();

        if !shader.activate(&mut pass, ctx) {
            return;
        }
        if let Some(idx) = shader.bind_groups().model {
            pass.set_bind_group(idx, data.uniform.bind_group(), &[]);
        }

        pass.draw(0..1, 0..self.particle_count);
    }

    fn priority(&self, _store: &AssetStore) -> u32 {
        if self.settings.opacity < 1.0 || self.settings.end_opacity < 1.0 {
            PROXY_PRIORITY_TRANSPARENT
        } else {
            PROXY_PRIORITY_SOLID
        }
    }
}
