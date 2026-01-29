use crate::assets::{AssetStore, H, HMaterial, HMesh, HShader, Shader};
use crate::core::bone::BoneData;
use crate::core::{BoundingSphere, ModelUniform};
use crate::math::Affine3A;
#[cfg(debug_assertions)]
use crate::rendering::DebugRenderer;
use crate::rendering::picking::hash_to_rgba;
use crate::rendering::proxies::{
    PROXY_PRIORITY_SOLID, PROXY_PRIORITY_TRANSPARENT, SceneProxy, SceneProxyBinding,
};
use crate::rendering::uniform::ShaderUniform;
use crate::rendering::{
    AssetCache, GPUDrawCtx, RenderPassType, Renderer, RuntimeMesh, RuntimeShader,
};
use crate::{proxy_data, proxy_data_mut, try_activate_shader};
use std::any::Any;
use std::ops::Range;
use std::sync::RwLockWriteGuard;
use syrillian_macros::UniformIndex;
use wgpu::RenderPass;

#[repr(u8)]
#[derive(Copy, Clone, Debug, UniformIndex)]
pub enum MeshUniformIndex {
    MeshData = 0,
    BoneData = 1,
}

#[derive(Debug, Clone)]
pub struct RuntimeMeshData {
    pub mesh_data: ModelUniform,
    // TODO: Consider having a uniform like that, for every Transform by default in some way, or
    //       lazy-make / provide one by default.
    pub uniform: ShaderUniform<MeshUniformIndex>,
}

#[derive(Debug, Clone)]
pub struct MeshSceneProxy {
    pub mesh: HMesh,
    pub materials: Vec<HMaterial>,
    pub material_ranges: Vec<Range<u32>>,
    pub bone_data: BoneData,
    pub bones_dirty: bool,
    pub bounding: BoundingSphere,
}

impl RuntimeMeshData {
    pub fn activate_shader(
        &self,
        shader: &RuntimeShader,
        ctx: &GPUDrawCtx,
        pass: &mut RenderPass,
    ) -> bool {
        try_activate_shader!(shader, pass, ctx => return false);

        if let Some(idx) = shader.bind_groups().model {
            pass.set_bind_group(idx, self.uniform.bind_group(), &[]);
        }

        true
    }
}

impl SceneProxy for MeshSceneProxy {
    fn setup_render(&mut self, renderer: &Renderer, local_to_world: &Affine3A) -> Box<dyn Any> {
        Box::new(self.setup_mesh_data(renderer, local_to_world))
    }

    fn update_render(
        &mut self,
        renderer: &Renderer,
        data: &mut dyn Any,
        local_to_world: &Affine3A,
    ) {
        let data: &mut RuntimeMeshData = proxy_data_mut!(data);

        // TODO: Consider Rigid Body render isometry interpolation for mesh local to world

        if self.bones_dirty {
            renderer.state.queue.write_buffer(
                data.uniform.buffer(MeshUniformIndex::BoneData),
                0,
                self.bone_data.as_bytes(),
            );
            self.bones_dirty = false;
        }

        data.mesh_data.model_mat = (*local_to_world).into();

        renderer.state.queue.write_buffer(
            data.uniform.buffer(MeshUniformIndex::MeshData),
            0,
            bytemuck::bytes_of(&data.mesh_data),
        );
    }

    fn render<'a>(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        let data: &RuntimeMeshData = proxy_data!(binding.proxy_data());

        let Some(mesh) = renderer.cache.mesh(self.mesh) else {
            return;
        };

        let mut pass = ctx.pass.write().unwrap();
        self.draw_mesh(ctx, &renderer.cache, &mesh, data, &mut pass);

        #[cfg(debug_assertions)]
        if !ctx.transparency_pass && DebugRenderer::mesh_edges() {
            draw_edges(ctx, &renderer.cache, &mesh, data, &mut pass);
        }

        #[cfg(debug_assertions)]
        if !ctx.transparency_pass && DebugRenderer::mesh_vertex_normals() {
            draw_vertex_normals(ctx, &renderer.cache, &mesh, data, &mut pass);
        }
    }

    fn render_shadows(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        let data: &RuntimeMeshData = proxy_data!(binding.proxy_data());

        let Some(mesh) = renderer.cache.mesh(self.mesh) else {
            return;
        };

        let mut pass = ctx.pass.write().unwrap();
        self.draw_mesh(ctx, &renderer.cache, &mesh, data, &mut pass);
    }

    // TODO: Make shaders more modular so picking and (shadow) shaders can be generated from just a vertex shader
    fn render_picking(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        debug_assert_ne!(ctx.pass_type, RenderPassType::Shadow);

        let data: &RuntimeMeshData = proxy_data!(binding.proxy_data());

        let Some(mesh) = renderer.cache.mesh(self.mesh) else {
            return;
        };

        let Some(mesh_data) = renderer.cache.meshes.store().try_get(self.mesh) else {
            return;
        };

        let mut pass = ctx.pass.write().unwrap();
        let shader = renderer.cache.shader(HShader::DIM3_PICKING);
        try_activate_shader!(shader, &mut pass, ctx => return);

        if let Some(model) = shader.bind_groups().model {
            pass.set_bind_group(model, data.uniform.bind_group(), &[]);
        }

        let color = hash_to_rgba(binding.object_hash);
        pass.set_immediates(0, bytemuck::bytes_of(&color));

        if mesh_data.material_ranges.is_empty() {
            mesh.draw_all(&mut pass);
            return;
        }

        for range in mesh_data.material_ranges.iter() {
            mesh.draw(range.clone(), &mut pass);
        }
    }

    fn priority(&self, store: &AssetStore) -> u32 {
        if self.materials.iter().any(|m| {
            let material = store.materials.get(*m);
            material.is_transparent()
        }) {
            PROXY_PRIORITY_TRANSPARENT
        } else {
            PROXY_PRIORITY_SOLID
        }
    }

    fn bounds(&self, local_to_world: &Affine3A) -> Option<BoundingSphere> {
        Some((self.bounding * 5.0).transformed(&(*local_to_world).into()))
    }
}

impl MeshSceneProxy {
    fn draw_mesh(
        &self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        mesh: &RuntimeMesh,
        runtime: &RuntimeMeshData,
        pass: &mut RwLockWriteGuard<RenderPass>,
    ) {
        let current_shader = HShader::DIM3;
        let shader = cache.shader_3d();

        if !runtime.activate_shader(&shader, ctx, pass) {
            return;
        }

        self.draw_materials(ctx, cache, mesh, runtime, pass, current_shader);
    }

    fn draw_materials(
        &self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        mesh: &RuntimeMesh,
        runtime: &RuntimeMeshData,
        pass: &mut RwLockWriteGuard<RenderPass>,
        current_shader: H<Shader>,
    ) {
        for (i, range) in self.material_ranges.iter().enumerate() {
            let h_mat = self
                .materials
                .get(i)
                .cloned()
                .unwrap_or(HMaterial::FALLBACK);
            let material = cache.material(h_mat);

            if ctx.pass_type == RenderPassType::Color
                && material.data.has_transparency() ^ ctx.transparency_pass
            {
                continue; // either transparent in a non-transparency pass, or transparent in a non-transparency pass
            }

            if ctx.pass_type == RenderPassType::Shadow
                && (!material.data.has_cast_shadows() || material.data.has_transparency())
            {
                continue;
            }

            if material.shader != current_shader {
                let shader = cache.shader(material.shader);
                if !runtime.activate_shader(&shader, ctx, pass) {
                    return;
                }
            }

            if let Some(idx) = cache.shader(material.shader).bind_groups().material {
                pass.set_bind_group(idx, material.uniform.bind_group(), &[]);
            }

            mesh.draw(range.clone(), pass);
        }
    }

    fn setup_mesh_data(
        &mut self,
        renderer: &Renderer,
        local_to_world: &Affine3A,
    ) -> RuntimeMeshData {
        let device = &renderer.state.device;
        let model_bgl = renderer.cache.bgl_model();
        let mesh_data = ModelUniform::from_matrix(&(*local_to_world).into());

        let uniform = ShaderUniform::<MeshUniformIndex>::builder((*model_bgl).clone())
            .with_buffer_data(&mesh_data)
            .with_buffer_data_slice(self.bone_data.bones.as_slice())
            .build(device);

        RuntimeMeshData { mesh_data, uniform }
    }
}

#[cfg(debug_assertions)]
fn draw_edges(
    ctx: &GPUDrawCtx,
    cache: &AssetCache,
    mesh: &RuntimeMesh,
    runtime: &RuntimeMeshData,
    pass: &mut RenderPass,
) {
    use syrillian::math::Vec4;

    const COLOR: Vec4 = Vec4::new(1.0, 0.0, 1.0, 1.0);

    let shader = cache.shader(HShader::DEBUG_EDGES);
    if !runtime.activate_shader(&shader, ctx, pass) {
        return;
    }

    pass.set_immediates(0, bytemuck::bytes_of(&COLOR));

    mesh.draw_all(pass);
}

#[cfg(debug_assertions)]
fn draw_vertex_normals(
    ctx: &GPUDrawCtx,
    cache: &AssetCache,
    mesh: &RuntimeMesh,
    runtime: &RuntimeMeshData,
    pass: &mut RenderPass,
) {
    let shader = cache.shader(HShader::DEBUG_VERTEX_NORMALS);
    if !runtime.activate_shader(&shader, ctx, pass) {
        return;
    }

    mesh.draw_all_as_instances(0..2, pass);
}
