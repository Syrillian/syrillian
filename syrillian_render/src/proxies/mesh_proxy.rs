use crate::cache::{AssetCache, RuntimeMesh, RuntimeShader};
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
use crate::{proxy_data, proxy_data_mut, try_activate_shader};
use glamx::Affine3A;
use parking_lot::RwLockWriteGuard;
use std::any::Any;
use std::ops::Range;
use syrillian_asset::material::MeshSkinning;
use syrillian_asset::mesh::bone::BoneData;
use syrillian_asset::store::{AssetStore, H, Store};
use syrillian_asset::{
    HMaterialInstance, HMesh, HTexture2D, Material, MaterialInstance, Shader, Texture2D,
};
use syrillian_macros::UniformIndex;
use syrillian_shadergen::value::MaterialValue;
use syrillian_utils::BoundingSphere;
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
    pub materials: Vec<HMaterialInstance>,
    pub material_ranges: Vec<Range<u32>>,
    pub bone_data: BoneData,
    pub bones_dirty: bool,
    pub skinned: bool,
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
    fn setup_render(
        &mut self,
        renderer: &Renderer,
        local_to_world: &Affine3A,
    ) -> Box<dyn Any + Send> {
        Box::new(self.setup_mesh_data(renderer, local_to_world))
    }

    fn refresh_transform(
        &mut self,
        renderer: &Renderer,
        data: &mut (dyn Any + Send),
        local_to_world: &Affine3A,
    ) {
        let data: &mut RuntimeMeshData = proxy_data_mut!(data);

        data.mesh_data.model_mat = (*local_to_world).into();

        renderer.state.queue.write_buffer(
            data.uniform.buffer(MeshUniformIndex::MeshData),
            0,
            bytemuck::bytes_of(&data.mesh_data),
        );
    }

    fn update_render(
        &mut self,
        renderer: &Renderer,
        data: &mut (dyn Any + Send),
        _local_to_world: &Affine3A,
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
    }

    fn render<'a>(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        let data: &RuntimeMeshData = proxy_data!(binding.proxy_data());

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
    }

    fn render_shadows(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        let data: &RuntimeMeshData = proxy_data!(binding.proxy_data());

        let Some(mesh) = renderer.cache.mesh(self.mesh) else {
            return;
        };

        let mut pass = ctx.pass.write();
        self.draw_mesh_shadow(ctx, &renderer.cache, &mesh, data, &mut pass);
    }

    // TODO: Make shaders more modular so picking and (shadow) shaders can be generated from just a vertex shader
    fn render_picking(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        debug_assert_ne!(ctx.pass_type, RenderPassType::Shadow);

        let data: &RuntimeMeshData = proxy_data!(binding.proxy_data());

        let Some(mesh) = renderer.cache.mesh(self.mesh) else {
            return;
        };

        let mut pass = ctx.pass.write();

        let color = hash_to_rgba(binding.object_hash);
        pass.set_immediates(0, bytemuck::bytes_of(&color));

        self.draw_mesh_picking(ctx, &renderer.cache, &mesh, data, &mut pass);
    }

    fn priority(&self, store: &AssetStore) -> u32 {
        if self.materials.iter().any(|m| {
            let instance = store.material_instances.get(*m);
            let material = store.materials.get(instance.material).clone();
            instance_is_transparent(&instance, &material, &store.textures)
        }) {
            PROXY_PRIORITY_TRANSPARENT
        } else {
            PROXY_PRIORITY_SOLID
        }
    }

    fn bounds(&self, local_to_world: &Affine3A) -> Option<BoundingSphere> {
        Some(self.bounding.transformed(&(*local_to_world).into()))
    }
}

impl MeshSceneProxy {
    fn draw_mesh_base(
        &self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        mesh: &RuntimeMesh,
        runtime: &RuntimeMeshData,
        pass: &mut RwLockWriteGuard<RenderPass>,
    ) {
        self.draw_materials(ctx, cache, mesh, runtime, pass, RenderPassType::Color);
    }

    fn draw_mesh_shadow(
        &self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        mesh: &RuntimeMesh,
        runtime: &RuntimeMeshData,
        pass: &mut RwLockWriteGuard<RenderPass>,
    ) {
        self.draw_materials(ctx, cache, mesh, runtime, pass, RenderPassType::Shadow);
    }

    fn draw_mesh_picking(
        &self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        mesh: &RuntimeMesh,
        runtime: &RuntimeMeshData,
        pass: &mut RwLockWriteGuard<RenderPass>,
    ) {
        self.draw_materials(ctx, cache, mesh, runtime, pass, RenderPassType::Picking);
    }

    fn draw_materials(
        &self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        mesh: &RuntimeMesh,
        runtime: &RuntimeMeshData,
        pass: &mut RwLockWriteGuard<RenderPass>,
        pass_type: RenderPassType,
    ) {
        let skinning = if self.skinned {
            MeshSkinning::Skinned
        } else {
            MeshSkinning::Unskinned
        };
        let mut current_shader: Option<H<Shader>> = None;

        let ranges: Vec<Range<u32>> = if self.material_ranges.is_empty() {
            vec![Range {
                start: 0,
                end: mesh.total_point_count(),
            }]
        } else {
            self.material_ranges.clone()
        };

        for (i, range) in ranges.iter().enumerate() {
            let h_mat = self
                .materials
                .get(i)
                .cloned()
                .unwrap_or(HMaterialInstance::FALLBACK);
            let material = cache.material_instance(h_mat);
            let shader_set = match skinning {
                MeshSkinning::Skinned => material.shader_skinned,
                MeshSkinning::Unskinned => material.shader_unskinned,
            };

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
                if !runtime.activate_shader(&shader, ctx, pass) {
                    return;
                }
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

        let uniform = ShaderUniform::<MeshUniformIndex>::builder(model_bgl)
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
    use glamx::Vec4;
    use syrillian_asset::HShader;

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
    use syrillian_asset::HShader;

    let shader = cache.shader(HShader::DEBUG_VERTEX_NORMALS);
    if !runtime.activate_shader(&shader, ctx, pass) {
        return;
    }

    mesh.draw_all_as_instances(0..2, pass);
}

fn instance_value_f32(
    instance: &MaterialInstance,
    material: &Material,
    name: &str,
    fallback: f32,
) -> f32 {
    if let Some(v) = instance.value_f32(name) {
        v
    } else if let Some(MaterialValue::F32(v)) = material.layout().default_value(name) {
        *v
    } else {
        fallback
    }
}

fn instance_value_bool(
    instance: &MaterialInstance,
    material: &Material,
    name: &str,
    fallback: bool,
) -> bool {
    if let Some(v) = instance.value_bool(name) {
        v
    } else if let Some(MaterialValue::Bool(v)) = material.layout().default_value(name) {
        *v
    } else {
        fallback
    }
}

fn instance_is_transparent(
    instance: &MaterialInstance,
    material: &Material,
    textures: &Store<Texture2D>,
) -> bool {
    let alpha = instance_value_f32(instance, material, "alpha", 1.0);
    let has_transparency = instance_value_bool(instance, material, "has_transparency", false);

    let diffuse_handle = instance
        .textures
        .get("diffuse")
        .and_then(|v| *v)
        .or_else(|| material.layout().texture_fallback("diffuse"))
        .unwrap_or(HTexture2D::FALLBACK_DIFFUSE);
    let diffuse = textures.get(diffuse_handle);

    alpha < 1.0 || has_transparency || diffuse.has_transparency
}
