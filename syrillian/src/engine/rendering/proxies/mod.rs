use crate::assets::AssetStore;
use crate::components::TypedComponentId;
use crate::core::BoundingSphere;
use crate::core::ObjectHash;
use crate::rendering::{GPUDrawCtx, RenderPassType, Renderer};
use nalgebra::{Affine3, Matrix4};
use std::any::Any;
use std::fmt::Debug;

pub mod mesh_proxy;
pub mod text_proxy;

#[cfg(debug_assertions)]
pub mod debug_proxy;

pub use mesh_proxy::*;
pub use text_proxy::*;

#[cfg(debug_assertions)]
pub use debug_proxy::*;

#[macro_export]
macro_rules! proxy_data_mut {
    ($data:expr) => {
        if let Some(data) = ($data as &mut dyn std::any::Any).downcast_mut() {
            data
        } else {
            ::syrillian::utils::debug_panic!(
                "Could not downcast proxy data. The GPU data type did not match up."
            );
            return;
        }
    };
}

#[macro_export]
macro_rules! proxy_data {
    ($data:expr) => {
        if let Some(data) = ($data as &dyn std::any::Any).downcast_ref() {
            data
        } else {
            ::syrillian_utils::debug_panic!(
                "Could not downcast proxy data. The GPU data type did not match up."
            );
            return;
        }
    };
}

pub const PROXY_PRIORITY_SOLID: u32 = 99;
pub const PROXY_PRIORITY_TRANSPARENT: u32 = 999;

pub trait SceneProxy: Send + Any + Debug {
    fn setup_render(&mut self, renderer: &Renderer, local_to_world: &Matrix4<f32>) -> Box<dyn Any>;
    fn update_render(
        &mut self,
        renderer: &Renderer,
        data: &mut dyn Any,
        local_to_world: &Matrix4<f32>,
    );
    fn render(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding);

    fn render_shadows(
        &self,
        _renderer: &Renderer,
        _ctx: &GPUDrawCtx,
        _binding: &SceneProxyBinding,
    ) {
    }

    fn render_picking(
        &self,
        _renderer: &Renderer,
        _ctx: &GPUDrawCtx,
        _binding: &SceneProxyBinding,
    ) {
    }

    fn priority(&self, store: &AssetStore) -> u32;

    fn bounds(&self, _local_to_world: &Matrix4<f32>) -> Option<BoundingSphere> {
        None
    }
}

pub struct SceneProxyBinding {
    pub component_id: TypedComponentId,
    pub object_hash: ObjectHash,
    pub local_to_world: Affine3<f32>,
    proxy_data: Box<dyn Any>,
    pub proxy: Box<dyn SceneProxy>,
    pub enabled: bool,
}

impl SceneProxyBinding {
    pub fn new(
        component_id: TypedComponentId,
        object_hash: ObjectHash,
        local_to_world: Affine3<f32>,
        proxy_data: Box<dyn Any>,
        proxy: Box<dyn SceneProxy>,
    ) -> Self {
        Self {
            component_id,
            object_hash,
            local_to_world,
            proxy_data,
            proxy,
            enabled: true,
        }
    }

    pub fn proxy_data(&self) -> &dyn Any {
        self.proxy_data.as_ref()
    }

    pub fn update_transform(&mut self, local_to_world: Affine3<f32>) {
        self.local_to_world = local_to_world;
    }

    pub fn update(&mut self, renderer: &Renderer) {
        self.proxy.update_render(
            renderer,
            self.proxy_data.as_mut(),
            self.local_to_world.matrix(),
        );
    }

    pub fn bounds(&self) -> Option<BoundingSphere> {
        self.proxy.bounds(self.local_to_world.matrix())
    }

    pub fn render(&self, renderer: &Renderer, ctx: &GPUDrawCtx) {
        self.proxy.render(renderer, ctx, self);
    }

    pub fn render_shadows(&self, renderer: &Renderer, ctx: &GPUDrawCtx) {
        self.proxy.render_shadows(renderer, ctx, self);
    }

    pub fn render_picking(&self, renderer: &Renderer, ctx: &GPUDrawCtx) {
        self.proxy.render_picking(renderer, ctx, self)
    }

    pub fn render_by_pass(&self, renderer: &Renderer, ctx: &GPUDrawCtx) {
        match ctx.pass_type {
            RenderPassType::Color | RenderPassType::Color2D => {
                self.proxy.render(renderer, ctx, self)
            }
            RenderPassType::Shadow => self.proxy.render_shadows(renderer, ctx, self),
            RenderPassType::Picking | RenderPassType::PickingUi => {
                self.proxy.render_picking(renderer, ctx, self)
            }
        }
    }
}
