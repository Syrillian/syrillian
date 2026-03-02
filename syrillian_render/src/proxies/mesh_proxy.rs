// TODO: refactor

use crate::cache::mesh::{BindMeshBuffers, RenderMesh};
use crate::cache::{AssetCache, RuntimeShader};
use crate::model_uniform::ModelUniform;
use crate::proxies::{
    PROXY_PRIORITY_SOLID, PROXY_PRIORITY_TRANSPARENT, SceneProxy, SceneProxyBinding,
};
#[cfg(debug_assertions)]
use crate::rendering::debug_renderer::DebugRenderer;
use crate::rendering::picking::hash_to_rgba;
use crate::rendering::renderer::Renderer;
use crate::rendering::uniform::ShaderUniform;
use crate::rendering::{GPUDrawCtx, RenderPassType};
use crate::{proxy_data, proxy_data_mut};
use glamx::Affine3A;
use parking_lot::RwLockWriteGuard;
use std::any::Any;
use std::ops::Range;
use syrillian_asset::shader::ShaderType;
use syrillian_asset::store::H;
use syrillian_asset::{HMaterialInstance, HMesh, Shader};
use syrillian_macros::UniformIndex;
use syrillian_utils::BoundingSphere;
use wgpu::RenderPass;
use zerocopy::IntoBytes;

#[repr(u8)]
#[derive(Copy, Clone, Debug, UniformIndex)]
pub enum MeshUniformIndex {
    MeshData = 0,
}

#[derive(Debug, Clone)]
pub struct RenderMeshData {
    pub visible_mesh_data: ModelUniform,
    // TODO: Consider having a uniform like that, for every Transform by default in some way, or
    //       lazy-make / provide one by default.
    pub visible_uniform: ShaderUniform<MeshUniformIndex>,

    #[cfg(debug_assertions)]
    pub real_mesh_data: Option<ModelUniform>,
    #[cfg(debug_assertions)]
    pub real_uniform: Option<ShaderUniform<MeshUniformIndex>>,

    #[cfg(debug_assertions)]
    pub bounds_uniform: Option<ShaderUniform<MeshUniformIndex>>,
}

#[derive(Debug, Clone)]
pub struct MeshSceneProxy {
    pub mesh: HMesh,
    pub materials: Vec<HMaterialInstance>,
    pub material_ranges: Vec<Range<u32>>,
    pub bounding: Option<BoundingSphere>,
    pub model_bounding: Option<BoundingSphere>,
}

impl RenderMeshData {
    pub fn new(
        visible_mesh_data: ModelUniform,
        visible_uniform: ShaderUniform<MeshUniformIndex>,
    ) -> Self {
        Self {
            visible_mesh_data,
            visible_uniform,
            #[cfg(debug_assertions)]
            real_mesh_data: None,
            #[cfg(debug_assertions)]
            real_uniform: None,
            #[cfg(debug_assertions)]
            bounds_uniform: None,
        }
    }
    pub fn activate_shader(&self, shader: &RuntimeShader, ctx: &GPUDrawCtx, pass: &mut RenderPass) {
        shader.activate(pass, ctx);

        if let Some(idx) = shader.bind_groups().model {
            pass.set_bind_group(idx, self.visible_uniform.bind_group(), &[]);
        }
    }
}

impl SceneProxy for MeshSceneProxy {
    fn setup_render(
        &mut self,
        renderer: &Renderer,
        render_affine: Affine3A,
        world_affine: Option<Affine3A>,
    ) -> Box<dyn Any + Send> {
        self.update_model_bounds(&render_affine);

        Box::new(self.setup_mesh_data(renderer, render_affine, world_affine))
    }

    fn refresh_transform(
        &mut self,
        renderer: &Renderer,
        data: &mut (dyn Any + Send),
        render_affine: Affine3A,
        _world_affine: Option<Affine3A>,
    ) {
        let data: &mut RenderMeshData = proxy_data_mut!(data);

        data.visible_mesh_data.update(&render_affine);

        renderer.state.queue.write_buffer(
            data.visible_uniform.buffer(MeshUniformIndex::MeshData),
            0,
            data.visible_mesh_data.as_bytes(),
        );

        self.update_model_bounds(&render_affine);

        #[cfg(debug_assertions)]
        self.refresh_transform_debug(data, renderer, &render_affine, _world_affine.as_ref());
    }

    fn render<'a>(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        let data: &RenderMeshData = proxy_data!(binding.proxy_data());

        let Some(mesh) = renderer.cache.mesh(self.mesh) else {
            return;
        };

        let mut pass = ctx.pass.write();
        self.draw_mesh_base(ctx, &renderer.cache, &mesh, data, &mut pass);

        #[cfg(debug_assertions)]
        if !ctx.transparency_pass && DebugRenderer::mesh_edges() {
            draw_edges(ctx, &renderer.cache, &mesh, data, &mut pass);
        }

        #[cfg(debug_assertions)]
        if !ctx.transparency_pass && DebugRenderer::mesh_vertex_normals() {
            draw_vertex_normals(ctx, &renderer.cache, &mesh, data, &mut pass);
        }

        #[cfg(debug_assertions)]
        if !ctx.transparency_pass && DebugRenderer::mesh_bounds() {
            draw_bounds(ctx, &renderer.cache, data, &mut pass);
        }
    }

    fn render_shadows(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        let data: &RenderMeshData = proxy_data!(binding.proxy_data());

        let Some(mesh) = renderer.cache.mesh(self.mesh) else {
            return;
        };

        let mut pass = ctx.pass.write();
        self.draw_mesh_shadow(ctx, &renderer.cache, &mesh, data, &mut pass);
    }

    fn render_picking(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        debug_assert_ne!(ctx.pass_type, RenderPassType::Shadow);

        let data: &RenderMeshData = proxy_data!(binding.proxy_data());

        let Some(mesh) = renderer.cache.mesh(self.mesh) else {
            return;
        };

        let mut picking_uniform = data.visible_mesh_data;
        picking_uniform.object_hash = hash_to_rgba(binding.object_hash);
        renderer.state.queue.write_buffer(
            data.visible_uniform.buffer(MeshUniformIndex::MeshData),
            0,
            picking_uniform.as_bytes(),
        );

        let mut pass = ctx.pass.write();

        self.draw_mesh_picking(ctx, &renderer.cache, &mesh, data, &mut pass);
    }

    fn priority(&self, cache: Option<&AssetCache>) -> u32 {
        let Some(cache) = cache else {
            return PROXY_PRIORITY_SOLID;
        };

        if self.materials.iter().any(|m| {
            cache
                .material_instances
                .inspect(*m, |m| m.transparent)
                .unwrap_or(false)
        }) {
            PROXY_PRIORITY_TRANSPARENT
        } else {
            PROXY_PRIORITY_SOLID
        }
    }

    fn bounds(&self) -> Option<BoundingSphere> {
        self.model_bounding
    }
}

impl MeshSceneProxy {
    #[inline]
    fn draw_mesh_base(
        &self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        mesh: &RenderMesh,
        runtime: &RenderMeshData,
        pass: &mut RwLockWriteGuard<RenderPass>,
    ) {
        self.draw_materials(ctx, cache, mesh, runtime, pass, RenderPassType::Color);
    }

    #[inline]
    fn draw_mesh_shadow(
        &self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        mesh: &RenderMesh,
        runtime: &RenderMeshData,
        pass: &mut RwLockWriteGuard<RenderPass>,
    ) {
        self.draw_materials(ctx, cache, mesh, runtime, pass, RenderPassType::Shadow);
    }

    #[inline]
    fn draw_mesh_picking(
        &self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        mesh: &RenderMesh,
        runtime: &RenderMeshData,
        pass: &mut RwLockWriteGuard<RenderPass>,
    ) {
        self.draw_materials(ctx, cache, mesh, runtime, pass, RenderPassType::Picking);
    }

    fn draw_materials(
        &self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        mesh: &RenderMesh,
        runtime: &RenderMeshData,
        pass: &mut RwLockWriteGuard<RenderPass>,
        pass_type: RenderPassType,
    ) {
        let mut current_shader: Option<H<Shader>> = None;

        let ranges: &[Range<u32>] = if self.material_ranges.is_empty() {
            &[Range {
                start: 0,
                end: mesh.total_point_count(),
            }]
        } else {
            &self.material_ranges
        };

        for (i, range) in ranges.iter().enumerate() {
            let h_mat = self
                .materials
                .get(i)
                .copied()
                .unwrap_or(HMaterialInstance::DEFAULT);
            let material = cache.material_instance(h_mat);
            let shader_set = material.shader_set;

            let target_shader = match pass_type {
                RenderPassType::Picking | RenderPassType::PickingUi => shader_set.picking,
                RenderPassType::Shadow => shader_set.shadow,
                _ => shader_set.base,
            };

            if pass_type == RenderPassType::Color && material.transparent ^ ctx.transparency_pass {
                continue; // either transparent in a non-transparency pass, or non-transparent in a transparency pass
            }

            if pass_type == RenderPassType::Shadow
                && (!material.cast_shadows || material.transparent)
            {
                continue;
            }

            let shader = cache.shader(target_shader);

            if current_shader != Some(target_shader) {
                runtime.activate_shader(&shader, ctx, pass);
                current_shader = Some(target_shader);
            }

            if let Some(idx) = shader.bind_groups().material {
                pass.set_bind_group(idx, &material.bind_group, &[]);
            }

            if pass_type == RenderPassType::Color && shader.immediate_size > 0 {
                debug_assert_eq!(
                    shader.immediate_size as usize,
                    material.immediates.len(),
                    "Immediate size of shader and material did not match. Shader requested {}, but material only supplied {}",
                    shader.immediate_size,
                    material.immediates.len()
                );

                pass.set_immediates(0, &material.immediates);
            }

            let mesh_buffers = match shader.shader_type {
                ShaderType::Picking | ShaderType::Shadow if shader.is_opaque() => {
                    BindMeshBuffers::POSITION
                }
                _ => BindMeshBuffers::all(),
            };

            mesh.draw(range.clone(), pass, mesh_buffers);
        }
    }

    fn setup_mesh_data(
        &mut self,
        renderer: &Renderer,
        render_affine: Affine3A,
        real_affine: Option<Affine3A>,
    ) -> RenderMeshData {
        let device = &renderer.state.device;
        let model_bgl = renderer.cache.bgl_model();
        let visible_mesh_data = ModelUniform::from_affine(&render_affine);

        let visible_uniform = ShaderUniform::<MeshUniformIndex>::builder(model_bgl.clone())
            .with_buffer_data(&visible_mesh_data)
            .build(device);

        #[cfg(debug_assertions)]
        let mut real_mesh_data = None;
        #[cfg(debug_assertions)]
        let mut real_uniform = None;
        #[cfg(debug_assertions)]
        if let Some(real_affine) = real_affine {
            let mesh_data = ModelUniform::from_affine(&real_affine);
            real_uniform = Some(
                ShaderUniform::<MeshUniformIndex>::builder(model_bgl.clone())
                    .with_buffer_data(&mesh_data)
                    .build(device),
            );
            real_mesh_data = Some(mesh_data);
        }

        #[cfg(debug_assertions)]
        let bounds_uniform = self
            .bounds_model_uniform(&render_affine)
            .map(|bounds_data| {
                ShaderUniform::<MeshUniformIndex>::builder(model_bgl.clone())
                    .with_buffer_data(&bounds_data)
                    .build(device)
            });

        RenderMeshData {
            visible_mesh_data,
            visible_uniform,
            #[cfg(debug_assertions)]
            bounds_uniform,
            #[cfg(debug_assertions)]
            real_mesh_data,
            #[cfg(debug_assertions)]
            real_uniform,
        }
    }

    fn update_model_bounds(&mut self, render_affine: &Affine3A) {
        let new_bounds = self
            .bounding
            .map(|b| b.transformed(&(*render_affine).into()));

        self.model_bounding = new_bounds;
    }

    #[cfg(debug_assertions)]
    fn bounds_model_uniform(&self, local_to_world: &Affine3A) -> Option<ModelUniform> {
        let bounds = self
            .bounding
            .map(|b| b.transformed(&(*local_to_world).into()))?;
        let radius = bounds.radius.abs().max(f32::EPSILON);
        let transform = glamx::Mat4::from_scale_rotation_translation(
            glamx::Vec3::splat(radius),
            glamx::Quat::IDENTITY,
            bounds.center,
        );
        Some(ModelUniform::from_matrix(&transform))
    }

    #[cfg(debug_assertions)]
    fn refresh_transform_debug(
        &self,
        data: &mut RenderMeshData,
        renderer: &Renderer,
        render_affine: &Affine3A,
        world_affine: Option<&Affine3A>,
    ) {
        if let Some(world_affine) = world_affine {
            let mesh_data = ModelUniform::from_affine(world_affine);
            if let Some(real_uniform) = data.real_uniform.as_mut() {
                real_uniform.write_buffer(
                    MeshUniformIndex::MeshData,
                    &mesh_data,
                    &renderer.state.queue,
                );
            } else {
                data.real_uniform = Some(
                    ShaderUniform::<MeshUniformIndex>::builder(renderer.cache.bgl_model().clone())
                        .with_buffer_data(&mesh_data)
                        .build(&renderer.state.device),
                );
            };
            data.real_mesh_data = Some(mesh_data);
        } else {
            data.real_uniform = None;
            data.real_mesh_data = None;
        }

        if let Some(bounds_uniform) = &data.bounds_uniform
            && let Some(bounds_data) = self.bounds_model_uniform(render_affine)
        {
            bounds_uniform.write_buffer(
                MeshUniformIndex::MeshData,
                &bounds_data,
                &renderer.state.queue,
            );
        }
    }
}

#[cfg(debug_assertions)]
fn draw_edges(
    ctx: &GPUDrawCtx,
    cache: &AssetCache,
    mesh: &RenderMesh,
    runtime: &RenderMeshData,
    pass: &mut RenderPass,
) {
    use glamx::Vec4;
    use syrillian_asset::HShader;

    const COLOR: Vec4 = Vec4::new(1.0, 0.0, 1.0, 1.0);
    const REAL_COLOR: Vec4 = Vec4::new(1.0, 0.2, 0.2, 1.0);

    let shader = cache.shader(HShader::DEBUG_EDGES);
    runtime.activate_shader(&shader, ctx, pass);

    pass.set_immediates(0, COLOR.as_bytes());
    mesh.draw_all(pass, BindMeshBuffers::POSITION);

    if let Some(real_uniform) = &runtime.real_uniform {
        pass.set_immediates(0, REAL_COLOR.as_bytes());

        if let Some(model) = shader.bind_groups().model {
            pass.set_bind_group(model, real_uniform.bind_group(), &[]);
        }

        mesh.draw_all(pass, BindMeshBuffers::POSITION);
    }
}

#[cfg(debug_assertions)]
fn draw_vertex_normals(
    ctx: &GPUDrawCtx,
    cache: &AssetCache,
    mesh: &RenderMesh,
    runtime: &RenderMeshData,
    pass: &mut RenderPass,
) {
    use syrillian_asset::HShader;

    let shader = cache.shader(HShader::DEBUG_VERTEX_NORMALS);
    runtime.activate_shader(&shader, ctx, pass);

    mesh.draw_all_as_instances(0..2, pass, BindMeshBuffers::POSITION_NORMAL);
}

#[cfg(debug_assertions)]
fn draw_bounds(
    ctx: &GPUDrawCtx,
    cache: &AssetCache,
    runtime: &RenderMeshData,
    pass: &mut RenderPass,
) {
    use glamx::Vec4;
    use syrillian_asset::{HMesh, HShader};

    const COLOR: Vec4 = Vec4::new(0.2, 1.0, 1.0, 1.0);

    let Some(bounds_uniform) = &runtime.bounds_uniform else {
        return;
    };
    let Some(bounds_mesh) = cache.mesh(HMesh::BOUNDS_GIZMO) else {
        return;
    };

    let shader = cache.shader(HShader::DEBUG_MESH_BOUNDS);
    runtime.activate_shader(&shader, ctx, pass);
    pass.set_immediates(0, COLOR.as_bytes());

    if let Some(idx) = shader.bind_groups().model {
        pass.set_bind_group(idx, bounds_uniform.bind_group(), &[]);
    }

    bounds_mesh.draw_all(pass, BindMeshBuffers::POSITION);
}
