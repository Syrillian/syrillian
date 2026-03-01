use crate::cache::generic_cache::CacheType;
use crate::cache::shader::bindings::ShaderBindings;
use crate::cache::{AssetCache, RenderPipelineBuilder};
use crate::rendering::GPUDrawCtx;
use crate::strobe::UiDrawContext;
use std::borrow::Cow;
use std::sync::Arc;
use syrillian_asset::Shader;
use syrillian_asset::shader::{BindGroupMap, ShaderType};
use wgpu::*;

mod bindings;
pub mod builder;

#[derive(Debug, Clone)]
pub struct RuntimeShader {
    name: String,
    pub module: ShaderModule,
    pipeline: RenderPipeline,
    pub immediate_size: u32,
    bind_groups: BindGroupMap,
    pub shader_type: ShaderType,
    opaque: bool,
}

impl CacheType for Shader {
    type Hot = Arc<RuntimeShader>;
    type UpdateMessage = Self;

    #[profiling::function]
    fn upload(this: Self, device: &Device, _queue: &Queue, cache: &AssetCache) -> Self::Hot {
        let bind_groups = this.bind_group_map();
        let code = this.gen_code_with_map(&bind_groups);

        debug_assert!(
            code.contains("@fragment") || this.stage() == ShaderType::Shadow,
            "No fragment entry point in non-shadow shader {:?}: \n{code}",
            this.name()
        );

        debug_assert!(
            code.contains("@vertex"),
            "No vertex entry point in shader {:?}: \n{code}",
            this.name()
        );

        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(this.name()),
            source: ShaderSource::Wgsl(Cow::Owned(code)),
        });
        let name = this.name().to_string();

        let solid_layout = this.pipeline_layout(device, cache);
        let solid_builder = RenderPipelineBuilder::builder(&this, &solid_layout, &module);
        let pipeline = solid_builder.build(device);

        Arc::new(RuntimeShader {
            name,
            module,
            pipeline,
            immediate_size: this.immediate_size(),
            bind_groups,
            shader_type: this.stage(),
            opaque: this.is_opaque(),
        })
    }
}

impl RuntimeShader {
    pub fn solid_pipeline(&self) -> &RenderPipeline {
        &self.pipeline
    }

    pub fn bind_groups(&self) -> &BindGroupMap {
        &self.bind_groups
    }

    pub fn is_opaque(&self) -> bool {
        self.opaque
    }

    pub fn activate(&self, pass: &mut RenderPass, ctx: &GPUDrawCtx) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(self.bind_groups.render, ctx.render_bind_group, &[]);
        if let Some(light) = self.bind_groups.light {
            pass.set_bind_group(light, ctx.light_bind_group, &[]);
        }
        if let Some(shadow) = self.bind_groups.shadow {
            pass.set_bind_group(shadow, ctx.shadow_bind_group, &[]);
        }
    }

    pub fn activate_ui(&self, pass: &mut RenderPass, ctx: &UiDrawContext) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(self.bind_groups.render, ctx.render_bind_group(), &[]);
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
