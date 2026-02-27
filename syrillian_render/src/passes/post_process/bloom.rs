use crate::passes::post_process::{PostProcessPass, PostProcessPassContext, PostProcessRoute};
use crate::rendering::offscreen_surface::OffscreenSurface;
use crate::rendering::uniform::ShaderUniform;
use syrillian_asset::{HComputeShader, ensure_aligned};
use syrillian_macros::UniformIndex;
use syrillian_utils::EngineArgs;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    AddressMode, BindGroupLayout, Buffer, BufferUsages, ComputePassDescriptor, Device, FilterMode,
    MipmapFilterMode, Queue, SamplerDescriptor, TextureFormat, TextureUsages, TextureView,
};
use zerocopy::IntoBytes;

#[derive(Debug, Copy, Clone)]
pub struct BloomSettings {
    pub enabled: bool,
    pub threshold: f32,
    pub soft_knee: f32,
    pub intensity: f32,
    pub radius: f32,
    pub clamp_max: f32,
    pub blur_passes: u32,
}

impl Default for BloomSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 1.0,
            soft_knee: 0.5,
            intensity: 0.25,
            radius: 1.0,
            clamp_max: 10.0,
            blur_passes: 2,
        }
    }
}

impl BloomSettings {
    pub fn sanitized(mut self) -> Self {
        self.threshold = self.threshold.clamp(0.0, 10.0);
        self.soft_knee = self.soft_knee.clamp(0.0, 1.0);
        self.intensity = self.intensity.clamp(0.0, 4.0);
        self.radius = self.radius.clamp(0.25, 8.0);
        self.clamp_max = self.clamp_max.clamp(0.0, 64.0);
        self.blur_passes = self.blur_passes.clamp(1, 6);
        self
    }

    pub fn from_engine_args() -> Self {
        let args = EngineArgs::get();
        let mut out = Self {
            enabled: !args.no_bloom,
            ..Default::default()
        };

        if let Some(v) = args.bloom_threshold {
            out.threshold = v;
        }
        if let Some(v) = args.bloom_soft_knee {
            out.soft_knee = v;
        }
        if let Some(v) = args.bloom_intensity {
            out.intensity = v;
        }
        if let Some(v) = args.bloom_radius {
            out.radius = v;
        }
        if let Some(v) = args.bloom_clamp_max {
            out.clamp_max = v;
        }
        if let Some(v) = args.bloom_blur_passes {
            out.blur_passes = v;
        }

        out.sanitized()
    }
}

#[repr(C)]
#[derive(
    Debug,
    Copy,
    Clone,
    zerocopy::Immutable,
    zerocopy::IntoBytes,
    zerocopy::FromBytes,
    zerocopy::KnownLayout,
)]
struct BloomComputeParams {
    threshold: f32,
    soft_knee: f32,
    intensity: f32,
    radius: f32,
    clamp_max: f32,
    _pad0: f32,
    direction: [f32; 2],
    texel_size: [f32; 2],
}

ensure_aligned!(
    BloomComputeParams {
        direction,
        texel_size
    },
    align <= 4 * 10 => size
);

impl BloomComputeParams {
    fn new(settings: &BloomSettings, direction: [f32; 2], texel_size: [f32; 2]) -> Self {
        Self {
            threshold: settings.threshold,
            soft_knee: settings.soft_knee,
            intensity: settings.intensity,
            radius: settings.radius,
            clamp_max: settings.clamp_max,
            _pad0: 0.0,
            direction,
            texel_size,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, UniformIndex)]
enum BloomComputeUniformIndex {
    Input = 0,
    AuxInput = 1,
    Sampler = 2,
    Params = 3,
    Output = 4,
}

pub struct BloomRenderPass {
    settings: BloomSettings,
    prefilter_params: Buffer,
    blur_horizontal_params: Buffer,
    blur_vertical_params: Buffer,
    composite_params: Buffer,
    prefilter_uniform: ShaderUniform<BloomComputeUniformIndex>,
    blur_horizontal_uniform: ShaderUniform<BloomComputeUniformIndex>,
    blur_vertical_uniform: ShaderUniform<BloomComputeUniformIndex>,
    composite_uniform: ShaderUniform<BloomComputeUniformIndex>,
    half_width: u32,
    half_height: u32,
    full_width: u32,
    full_height: u32,
    _half_a: OffscreenSurface,
    _half_b: OffscreenSurface,
}

impl BloomRenderPass {
    pub fn new(
        device: &Device,
        width: u32,
        height: u32,
        bloom_compute_bgl: BindGroupLayout,
        route: &PostProcessRoute,
        settings: &BloomSettings,
    ) -> Self {
        let full_width = width.max(1);
        let full_height = height.max(1);
        let half_width = full_width.div_ceil(2);
        let half_height = full_height.div_ceil(2);

        let half_a = OffscreenSurface::new_sized_with(
            device,
            half_width,
            half_height,
            TextureFormat::Rgba8Unorm,
            TextureUsages::STORAGE_BINDING,
        );
        let half_b = OffscreenSurface::new_sized_with(
            device,
            half_width,
            half_height,
            TextureFormat::Rgba8Unorm,
            TextureUsages::STORAGE_BINDING,
        );

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Bloom Compute Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Linear,
            ..SamplerDescriptor::default()
        });

        let settings = settings.sanitized();

        let params_prefilter = BloomComputeParams::new(
            &settings,
            [0.0, 0.0],
            [1.0 / full_width as f32, 1.0 / full_height as f32],
        );
        let params_blur_h = BloomComputeParams::new(
            &settings,
            [1.0, 0.0],
            [1.0 / half_width as f32, 1.0 / half_height as f32],
        );
        let params_blur_v = BloomComputeParams::new(
            &settings,
            [0.0, 1.0],
            [1.0 / half_width as f32, 1.0 / half_height as f32],
        );
        let params_composite = BloomComputeParams::new(
            &settings,
            [0.0, 0.0],
            [1.0 / full_width as f32, 1.0 / full_height as f32],
        );

        let prefilter_params = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Bloom Prefilter Params"),
            contents: params_prefilter.as_bytes(),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let blur_horizontal_params = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Bloom Blur Horizontal Params"),
            contents: params_blur_h.as_bytes(),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let blur_vertical_params = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Bloom Blur Vertical Params"),
            contents: params_blur_v.as_bytes(),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let composite_params = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Bloom Composite Params"),
            contents: params_composite.as_bytes(),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let prefilter_uniform =
            ShaderUniform::<BloomComputeUniformIndex>::builder(bloom_compute_bgl.clone())
                .with_texture(route.input_color.clone())
                .with_texture(route.input_color.clone())
                .with_sampler(sampler.clone())
                .with_buffer(prefilter_params.clone())
                .with_texture(half_a.view().clone())
                .build(device);

        let blur_horizontal_uniform =
            ShaderUniform::<BloomComputeUniformIndex>::builder(bloom_compute_bgl.clone())
                .with_texture(half_a.view().clone())
                .with_texture(half_a.view().clone())
                .with_sampler(sampler.clone())
                .with_buffer(blur_horizontal_params.clone())
                .with_texture(half_b.view().clone())
                .build(device);

        let blur_vertical_uniform =
            ShaderUniform::<BloomComputeUniformIndex>::builder(bloom_compute_bgl.clone())
                .with_texture(half_b.view().clone())
                .with_texture(half_b.view().clone())
                .with_sampler(sampler.clone())
                .with_buffer(blur_vertical_params.clone())
                .with_texture(half_a.view().clone())
                .build(device);

        let composite_uniform =
            ShaderUniform::<BloomComputeUniformIndex>::builder(bloom_compute_bgl)
                .with_texture(route.input_color.clone())
                .with_texture(half_a.view().clone())
                .with_sampler(sampler)
                .with_buffer(composite_params.clone())
                .with_texture(route.output_color.clone())
                .build(device);

        Self {
            settings,
            prefilter_params,
            blur_horizontal_params,
            blur_vertical_params,
            composite_params,
            prefilter_uniform,
            blur_horizontal_uniform,
            blur_vertical_uniform,
            composite_uniform,
            half_width,
            half_height,
            full_width,
            full_height,
            _half_a: half_a,
            _half_b: half_b,
        }
    }

    pub fn update_settings(&mut self, queue: &Queue, settings: &BloomSettings) {
        self.settings = settings.sanitized();

        let prefilter = BloomComputeParams::new(
            &self.settings,
            [0.0, 0.0],
            [
                1.0 / self.full_width.max(1) as f32,
                1.0 / self.full_height.max(1) as f32,
            ],
        );
        let blur_h = BloomComputeParams::new(
            &self.settings,
            [1.0, 0.0],
            [
                1.0 / self.half_width.max(1) as f32,
                1.0 / self.half_height.max(1) as f32,
            ],
        );
        let blur_v = BloomComputeParams::new(
            &self.settings,
            [0.0, 1.0],
            [
                1.0 / self.half_width.max(1) as f32,
                1.0 / self.half_height.max(1) as f32,
            ],
        );
        let composite = BloomComputeParams::new(
            &self.settings,
            [0.0, 0.0],
            [
                1.0 / self.full_width.max(1) as f32,
                1.0 / self.full_height.max(1) as f32,
            ],
        );

        queue.write_buffer(&self.prefilter_params, 0, prefilter.as_bytes());
        queue.write_buffer(&self.blur_horizontal_params, 0, blur_h.as_bytes());
        queue.write_buffer(&self.blur_vertical_params, 0, blur_v.as_bytes());
        queue.write_buffer(&self.composite_params, 0, composite.as_bytes());
    }
}

impl PostProcessPass for BloomRenderPass {
    fn name(&self) -> &'static str {
        "Bloom"
    }

    fn execute(&mut self, ctx: &mut PostProcessPassContext<'_>, _output_color: &TextureView) {
        let full_width = ctx.camera_render_data.system_data.screen_size.x.max(1);
        let full_height = ctx.camera_render_data.system_data.screen_size.y.max(1);
        let half_dispatch_x = self.half_width.div_ceil(8);
        let half_dispatch_y = self.half_height.div_ceil(8);
        let full_dispatch_x = full_width.div_ceil(8);
        let full_dispatch_y = full_height.div_ceil(8);

        let prefilter_shader = ctx
            .cache
            .compute_shader(HComputeShader::POST_PROCESS_BLOOM_PREFILTER);
        let blur_shader = ctx
            .cache
            .compute_shader(HComputeShader::POST_PROCESS_BLOOM_BLUR);
        let composite_shader = ctx
            .cache
            .compute_shader(HComputeShader::POST_PROCESS_BLOOM_COMPOSITE);

        let mut pass = ctx.encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Bloom Compute Pass"),
            ..ComputePassDescriptor::default()
        });

        pass.set_pipeline(prefilter_shader.pipeline());
        pass.set_bind_group(0, self.prefilter_uniform.bind_group(), &[]);
        pass.dispatch_workgroups(half_dispatch_x, half_dispatch_y, 1);

        pass.set_pipeline(blur_shader.pipeline());
        for _ in 0..self.settings.blur_passes {
            pass.set_bind_group(0, self.blur_horizontal_uniform.bind_group(), &[]);
            pass.dispatch_workgroups(half_dispatch_x, half_dispatch_y, 1);

            pass.set_bind_group(0, self.blur_vertical_uniform.bind_group(), &[]);
            pass.dispatch_workgroups(half_dispatch_x, half_dispatch_y, 1);
        }

        pass.set_pipeline(composite_shader.pipeline());
        pass.set_bind_group(0, self.composite_uniform.bind_group(), &[]);
        pass.dispatch_workgroups(full_dispatch_x, full_dispatch_y, 1);
    }
}
