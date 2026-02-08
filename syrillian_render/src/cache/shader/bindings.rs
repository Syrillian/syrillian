use crate::cache::AssetCache;
use syrillian_asset::{HBGL, Shader};
use wgpu::{BindGroupLayout, Device, PipelineLayout, PipelineLayoutDescriptor};

pub trait ShaderBindings {
    fn collect_bgls(&self, cache: &AssetCache) -> Vec<BindGroupLayout>;
    fn solid_layout(&self, device: &Device, cache: &AssetCache) -> PipelineLayout;
    fn shadow_layout(&self, device: &Device, cache: &AssetCache) -> Option<PipelineLayout>;
    fn layout_with(
        &self,
        device: &Device,
        layout_name: &str,
        fixed_bgls: &[&BindGroupLayout],
    ) -> PipelineLayout;
}

impl ShaderBindings for Shader {
    fn collect_bgls(&self, cache: &AssetCache) -> Vec<BindGroupLayout> {
        let mut out = Vec::new();
        out.push(cache.bgl_render().clone());

        if self.is_post_process() {
            out.push(cache.bgl_post_process().clone());
            return out;
        }

        if self.needs_bgl(HBGL::MODEL) {
            out.push(cache.bgl_model().clone());
        }
        if self.needs_bgl(HBGL::MATERIAL) {
            if let Some(layout) = self.material_layout() {
                out.push(cache.material_layout(layout));
            } else {
                out.push(cache.bgl_material().clone());
            }
        }
        if self.needs_bgl(HBGL::LIGHT) {
            out.push(cache.bgl_light());
        }
        if self.needs_bgl(HBGL::SHADOW) {
            out.push(cache.bgl_shadow());
        }

        out
    }

    fn solid_layout(&self, device: &Device, cache: &AssetCache) -> PipelineLayout {
        let layout_name = format!("{} Pipeline Layout", self.name());
        let layouts = self.collect_bgls(cache);
        let refs: Vec<&BindGroupLayout> = layouts.iter().collect();

        self.layout_with(device, &layout_name, &refs)
    }

    fn shadow_layout(&self, device: &Device, cache: &AssetCache) -> Option<PipelineLayout> {
        if self.is_post_process() {
            return None;
        }

        let layout_name = format!("{} Shadow Pipeline Layout", self.name());
        let layouts = self.collect_bgls(cache);
        let refs: Vec<&BindGroupLayout> = layouts.iter().collect();

        Some(self.layout_with(device, &layout_name, &refs))
    }

    fn layout_with(
        &self,
        device: &Device,
        layout_name: &str,
        fixed_bgls: &[&BindGroupLayout],
    ) -> PipelineLayout {
        let desc = PipelineLayoutDescriptor {
            label: Some(layout_name),
            bind_group_layouts: fixed_bgls,
            immediate_size: self.immediate_size(),
        };
        device.create_pipeline_layout(&desc)
    }
}
