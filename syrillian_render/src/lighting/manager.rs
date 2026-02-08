use crate::cache::AssetCache;
use crate::lighting::proxy::{LightProxy, LightType, LightUniformIndex, ShadowUniformIndex};
use crate::rendering::message::LightProxyCommand;
use crate::rendering::render_data::RenderUniformData;
#[cfg(debug_assertions)]
use crate::rendering::renderer::Renderer;
use crate::rendering::uniform::ShaderUniform;
#[cfg(debug_assertions)]
use crate::try_activate_shader;
use itertools::Itertools;
use syrillian_asset::store::{Ref, Store, StoreType};
use syrillian_asset::{HRenderTexture2DArray, RenderTexture2DArray};
use syrillian_utils::{TypedComponentId, debug_panic};
use tracing::{trace, warn};
use wgpu::{
    AddressMode, Device, FilterMode, MipmapFilterMode, Queue, Sampler, SamplerDescriptor,
    TextureUsages, TextureView, TextureViewDescriptor,
};

const DUMMY_POINT_LIGHT: LightProxy = LightProxy::dummy();

pub struct LightManager {
    proxy_owners: Vec<TypedComponentId>,
    proxies: Vec<LightProxy>,
    shadow_assignments: Vec<ShadowAssignment>,
    render_data: Vec<RenderUniformData>,

    uniform: ShaderUniform<LightUniformIndex>,
    shadow_uniform: ShaderUniform<ShadowUniformIndex>,
    empty_shadow_uniform: ShaderUniform<ShadowUniformIndex>,
    pub shadow_texture: HRenderTexture2DArray,
    pub _shadow_sampler: Sampler,
}

#[derive(Debug, Copy, Clone)]
pub struct ShadowAssignment {
    pub layer: u32,
    pub light_index: usize,
    pub face: u8,
}

impl LightManager {
    #[profiling::function]
    pub fn update_shadow_map_ids(
        &mut self,
        layers: u32,
        device: &Device,
        cache: &AssetCache,
    ) -> u32 {
        self.shadow_assignments.clear();

        let render_bgl = cache.bgl_render();

        let mut next_layer = 0;
        for (idx, light) in self.proxies.iter_mut().enumerate() {
            let light_type = LightType::try_from(light.type_id).unwrap_or(LightType::Point);
            let required_layers = match light_type {
                LightType::Point => 6,
                LightType::Spot => 1,
                LightType::Sun => 0,
            };

            if required_layers == 0 {
                light.shadow_map_id = u32::MAX;
                continue;
            }

            if next_layer + required_layers > layers {
                light.shadow_map_id = u32::MAX;
                continue;
            }

            light.shadow_map_id = next_layer;

            for face in 0..required_layers {
                self.shadow_assignments.push(ShadowAssignment {
                    layer: next_layer + face,
                    light_index: idx,
                    face: face as u8,
                });
            }

            while self.shadow_assignments.len() >= self.render_data.len() {
                profiling::scope!("add render uniform");
                self.render_data
                    .push(RenderUniformData::empty(device, &render_bgl));
            }

            next_layer += required_layers;
        }

        next_layer
    }

    #[profiling::function]
    pub fn add_proxy(&mut self, owner: TypedComponentId, proxy: LightProxy) {
        trace!("Registered Light Proxy for #{:?}", owner.type_id());
        if let Some((idx, _)) = self
            .proxy_owners
            .iter()
            .find_position(|tcid| **tcid == owner)
        {
            self.proxies[idx] = proxy;
        } else {
            self.proxies.push(proxy);
            self.proxy_owners.push(owner);
        }
    }

    #[profiling::function]
    pub fn remove_proxy(&mut self, owner: TypedComponentId) {
        let Some((pos, _)) = self
            .proxy_owners
            .iter()
            .find_position(|tcid| **tcid == owner)
        else {
            return;
        };

        self.proxy_owners.remove(pos);
        self.proxies.remove(pos);
    }

    #[profiling::function]
    pub fn execute_light_command(&mut self, owner: TypedComponentId, cmd: LightProxyCommand) {
        let Some((pos, _)) = self
            .proxy_owners
            .iter()
            .find_position(|tcid| **tcid == owner)
        else {
            warn!("Requested Light Proxy not found");
            return;
        };

        let Some(proxy) = self.proxies.get_mut(pos) else {
            debug_panic!("Light Proxy and Light Owners desynchronized");
            return;
        };

        cmd(proxy);
    }

    pub fn shadow_array<'a>(
        &self,
        assets: &'a Store<RenderTexture2DArray>,
    ) -> Option<Ref<'a, RenderTexture2DArray>> {
        assets.try_get(self.shadow_texture)
    }

    pub fn shadow_layer(&self, cache: &AssetCache, layer: u32) -> Option<TextureView> {
        let texture = &cache.render_texture_array(self.shadow_texture)?.texture;
        Some(texture.create_view(&TextureViewDescriptor {
            label: Some("Shadow Map Layer"),
            format: Some(wgpu::TextureFormat::Depth32Float),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: layer,
            array_layer_count: Some(1),
            usage: Some(TextureUsages::RENDER_ATTACHMENT),
        }))
    }

    pub fn uniform(&self) -> &ShaderUniform<LightUniformIndex> {
        &self.uniform
    }

    pub fn placeholder_shadow_uniform(&self) -> &ShaderUniform<ShadowUniformIndex> {
        &self.empty_shadow_uniform
    }

    pub fn shadow_uniform(&self) -> &ShaderUniform<ShadowUniformIndex> {
        &self.shadow_uniform
    }

    pub fn shadow_assignments(&self) -> &[ShadowAssignment] {
        &self.shadow_assignments
    }

    pub fn light(&self, index: usize) -> Option<&LightProxy> {
        self.proxies.get(index)
    }

    #[profiling::function]
    pub fn new(cache: &AssetCache, device: &Device) -> Self {
        const DUMMY_POINT_LIGHT: LightProxy = LightProxy::dummy();

        let shadow_texture = RenderTexture2DArray::new_shadow_map(48, 1024, 1024).store(&cache);
        let empty_shadow_texture = RenderTexture2DArray::new_shadow_map(2, 1, 1).store(&cache);
        let texture = cache
            .render_texture_arrays
            .try_get(shadow_texture, cache)
            .unwrap();
        let empty_texture = cache
            .render_texture_arrays
            .try_get(empty_shadow_texture, cache)
            .unwrap();

        let bgl = cache.bgl_light();
        let count: u32 = 0;
        let uniform = ShaderUniform::builder(bgl)
            .with_buffer_data(&count)
            .with_storage_buffer_data(&[DUMMY_POINT_LIGHT])
            .build(device);

        let shadow_sampler = device.create_sampler(&SamplerDescriptor {
            label: None,
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            compare: Some(wgpu::CompareFunction::LessEqual),
            anisotropy_clamp: 1,
            border_color: None,
        });

        let bgl = cache.bgl_shadow();
        let shadow_uniform = ShaderUniform::builder(bgl.clone())
            .with_texture(texture.view.clone())
            .with_sampler(shadow_sampler.clone())
            .build(device);

        let empty_shadow_uniform = ShaderUniform::builder(bgl)
            .with_texture(empty_texture.view.clone())
            .with_sampler(shadow_sampler.clone())
            .build(device);

        Self {
            proxy_owners: vec![],
            proxies: vec![],
            shadow_assignments: vec![],
            render_data: vec![],
            uniform,
            shadow_uniform,
            empty_shadow_uniform,
            shadow_texture,
            _shadow_sampler: shadow_sampler,
        }
    }

    #[profiling::function]
    pub fn update(&mut self, cache: &AssetCache, queue: &Queue, device: &Device) {
        for (render_data, assignment) in self
            .render_data
            .iter_mut()
            .zip(self.shadow_assignments.iter())
        {
            let Some(light) = self.proxies.get_mut(assignment.light_index) else {
                debug_panic!("Invalid Light Index was stored");
                continue;
            };

            let Ok(light_type) = LightType::try_from(light.type_id) else {
                debug_panic!("Invalid Light Type Id was stored");
                continue;
            };

            match light_type {
                LightType::Point => {
                    render_data.update_shadow_camera_for_point(light, assignment.face, queue)
                }
                LightType::Spot => render_data.update_shadow_camera_for_spot(light, queue),
                LightType::Sun => (),
            }
        }

        let proxies = proxy_buffer_slice(&self.proxies);
        let size = proxies.len();

        let count = self.uniform.buffer(LightUniformIndex::Count);
        queue.write_buffer(count, 0, bytemuck::bytes_of(&(size as u32)));

        let data = self.uniform.buffer(LightUniformIndex::Lights);
        if size_of_val(proxies) > data.size() as usize {
            let bgl = cache.bgl_light();
            self.uniform = ShaderUniform::builder(bgl)
                .with_buffer(count.clone())
                .with_storage_buffer_data(proxies)
                .build(device);
        } else {
            queue.write_buffer(data, 0, bytemuck::cast_slice(proxies));
        }
    }

    #[cfg(debug_assertions)]
    pub fn render_debug_lights(&self, renderer: &Renderer, ctx: &crate::rendering::GPUDrawCtx) {
        use syrillian_asset::HShader;

        let mut pass = ctx.pass.write();

        let shader = renderer.cache.shader(HShader::DEBUG_LIGHT);
        try_activate_shader!(shader, &mut pass, ctx => return);

        let lights = self.proxies.as_slice();
        for (i, proxy) in lights.iter().enumerate().take(self.proxies.len()) {
            let type_id: LightType = match proxy.type_id.try_into() {
                Ok(ty) => ty,
                Err(e) => {
                    debug_panic!("{}", e);
                    continue;
                }
            };

            pass.set_immediates(0, bytemuck::bytes_of(&(i as u32)));
            match type_id {
                LightType::Point => pass.draw(0..2, 0..6),
                LightType::Sun => pass.draw(0..2, 0..9),
                LightType::Spot => pass.draw(0..2, 0..9),
            }
        }
    }

    pub fn render_data(&self, assignment: u32) -> Option<&RenderUniformData> {
        self.render_data.get(assignment as usize)
    }

    pub fn all_render_data(&self) -> &[RenderUniformData] {
        &self.render_data
    }
}

pub fn proxy_buffer_slice(proxies: &[LightProxy]) -> &[LightProxy] {
    if proxies.is_empty() {
        &[DUMMY_POINT_LIGHT]
    } else {
        proxies
    }
}
