use crate::assets::ShaderType;
use crate::engine::assets::{BindGroupMap, Shader};
use crate::engine::rendering::cache::AssetCache;
use crate::engine::rendering::cache::generic_cache::CacheType;
use crate::rendering::{GPUDrawCtx, RenderPassType, RenderPipelineBuilder};
use std::borrow::Cow;
use std::sync::Arc;
use wgpu::*;

pub mod builder;

#[derive(Debug, Clone)]
pub struct RuntimeShader {
    name: String,
    pub module: ShaderModule,
    pipeline: RenderPipeline,
    shadow_pipeline: Option<RenderPipeline>,
    pub immediate_size: u32,
    bind_groups: BindGroupMap,
    pub shader_type: ShaderType,
}

impl CacheType for Shader {
    type Hot = Arc<RuntimeShader>;

    fn upload(self, device: &Device, _queue: &Queue, cache: &AssetCache) -> Self::Hot {
        let bind_groups = self.bind_group_map();
        let code = self.gen_code_with_map(&bind_groups);

        debug_assert!(
            code.contains("@fragment"),
            "No fragment entry point in shader {:?}: \n{code}",
            self.name()
        );
        debug_assert!(
            code.contains("@vertex"),
            "No vertex entry point in shader {:?}: \n{code}",
            self.name()
        );

        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(self.name()),
            source: ShaderSource::Wgsl(Cow::Owned(code)),
        });
        let name = self.name().to_string();

        let solid_layout = self.solid_layout(device, cache);
        let solid_builder = RenderPipelineBuilder::builder(&self, &solid_layout, &module);
        let pipeline = solid_builder.build(device);
        let shadow_pipeline = self.shadow_layout(device, cache).and_then(|layout| {
            let shadow_builder = RenderPipelineBuilder::builder(&self, &layout, &module);
            shadow_builder.build_shadow(device)
        });

        Arc::new(RuntimeShader {
            name,
            module,
            pipeline,
            shadow_pipeline,
            immediate_size: self.immediate_size(),
            bind_groups,
            shader_type: self.stage(),
        })
    }
}

impl RuntimeShader {
    pub fn solid_pipeline(&self) -> &RenderPipeline {
        &self.pipeline
    }

    pub fn shadow_pipeline(&self) -> Option<&RenderPipeline> {
        self.shadow_pipeline.as_ref()
    }

    pub fn pipeline(&self, stage: RenderPassType) -> Option<&RenderPipeline> {
        match stage {
            RenderPassType::Color
            | RenderPassType::Color2D
            | RenderPassType::Picking
            | RenderPassType::PickingUi => Some(&self.pipeline),
            RenderPassType::Shadow => self.shadow_pipeline.as_ref(),
        }
    }

    pub fn bind_groups(&self) -> &BindGroupMap {
        &self.bind_groups
    }

    pub fn activate(&self, pass: &mut RenderPass, ctx: &GPUDrawCtx) -> bool {
        crate::must_pipeline!(pipeline = self, ctx.pass_type => return false);

        pass.set_pipeline(pipeline);
        pass.set_bind_group(self.bind_groups.render, ctx.render_bind_group, &[]);
        if let Some(light) = self.bind_groups.light {
            pass.set_bind_group(light, ctx.light_bind_group, &[]);
        }
        if let Some(shadow) = self.bind_groups.shadow {
            pass.set_bind_group(shadow, ctx.shadow_bind_group, &[]);
        }

        true
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[macro_export]
macro_rules! activate_shader {
    ($shader:expr, $pass:expr, $ctx:expr => $( $exit_strat:tt )*) => {
        if !$shader.activate($pass, $ctx) {
            ::syrillian_utils::debug_panic!(
                "Invalid pipeline for specified shader. Cannot activate shader."
            );
            ::tracing::error!("A pipeline for the specified shader could not be found for the current render pass");
            $( $exit_strat )*;
        };
    };
}

#[macro_export]
macro_rules! try_activate_shader {
    ($shader:expr, $pass:expr, $ctx:expr => $( $exit_strat:tt )*) => {
        if !$shader.activate($pass, $ctx) {
            ::tracing::debug!("Tried to activate shader {:?}, but pipeline was not found for the specified render pass of type {:?}", $shader.name(), $ctx.pass_type);
            $( $exit_strat )*;
        };
    };
}

#[macro_export]
macro_rules! must_pipeline {
    ($name:ident = $shader:expr, $pass_type:expr => $( $exit_strat:tt )*) => {
        let Some($name) = $shader.pipeline($pass_type) else {
            ::syrillian_utils::debug_panic!(
                "A 3D Shader was instantiated without a Shadow Pipeline Variant"
            );
            ::tracing::error!("A 3D Shader was instantiated without a Shadow Pipeline Variant");
            $( $exit_strat )*;
        };
    };
}
