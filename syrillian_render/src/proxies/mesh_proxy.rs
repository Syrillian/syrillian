// TODO: refactor

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
use std::mem::size_of;
use std::ops::Range;
use syrillian_asset::mesh::Vertex3D;
use syrillian_asset::mesh::bone::BoneData;
use syrillian_asset::store::{AssetStore, H, Store};
use syrillian_asset::{
    HComputeShader, HMaterialInstance, HMesh, HTexture2D, Material, MaterialInstance, Shader,
    Texture2D,
};
use syrillian_macros::UniformIndex;
use syrillian_shadergen::value::MaterialValue;
use syrillian_utils::BoundingSphere;
use wgpu::{Buffer, BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor, RenderPass};

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
    pub skinned_buffers: Vec<Buffer>,
    pub skinning_uniforms: Vec<ShaderUniform<MeshSkinningComputeUniformIndex>>,
    pub skinning_vertex_counts: Vec<u32>,
    pub skinning_mesh: Option<HMesh>,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MeshSkinningParams {
    vertex_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, UniformIndex)]
pub enum MeshSkinningComputeUniformIndex {
    Bones = 0,
    Params = 1,
    Source = 2,
    Dest = 3,
}

#[derive(Debug, Clone)]
pub struct MeshSceneProxy {
    pub mesh: HMesh,
    pub materials: Vec<HMaterialInstance>,
    pub material_ranges: Vec<Range<u32>>,
    pub bone_data: BoneData,
    pub bones_dirty: bool,
    pub skinned: bool,
    pub bounding: Option<BoundingSphere>,
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

    fn ensure_skinning_runtime(&mut self, renderer: &Renderer, mesh_handle: HMesh) -> bool {
        let valid_existing = self.skinning_mesh == Some(mesh_handle)
            && self.skinned_buffers.len() == self.skinning_uniforms.len()
            && self.skinned_buffers.len() == self.skinning_vertex_counts.len()
            && !self.skinned_buffers.is_empty();
        if valid_existing {
            return false;
        }

        self.skinned_buffers.clear();
        self.skinning_uniforms.clear();
        self.skinning_vertex_counts.clear();
        self.skinning_mesh = Some(mesh_handle);

        let Some(mesh) = renderer.cache.mesh(mesh_handle) else {
            return false;
        };

        let device = &renderer.state.device;
        let skinning_bgl = renderer.cache.bgl_mesh_skinning_compute();
        let bone_buffer = self.uniform.buffer(MeshUniformIndex::BoneData).clone();

        for meshlet in mesh.meshlets() {
            let vertex_count = meshlet.vertex_count;
            let params = MeshSkinningParams {
                vertex_count,
                _pad0: 0,
                _pad1: 0,
                _pad2: 0,
            };
            let output_size = ((vertex_count as u64) * size_of::<Vertex3D>() as u64).max(4);
            let output = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Skinned Mesh Vertex Buffer"),
                size: output_size,
                usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let uniform =
                ShaderUniform::<MeshSkinningComputeUniformIndex>::builder(skinning_bgl.clone())
                    .with_buffer(bone_buffer.clone())
                    .with_buffer_data(&params)
                    .with_storage_buffer(meshlet.vertex_buffer.clone())
                    .with_storage_buffer(output.clone())
                    .build(device);

            self.skinned_buffers.push(output);
            self.skinning_uniforms.push(uniform);
            self.skinning_vertex_counts.push(vertex_count);
        }

        true
    }

    // TODO: Improve dispatching to be centralized so the driver can batch better
    fn dispatch_skinning(&self, renderer: &Renderer) {
        if self.skinning_uniforms.is_empty() {
            return;
        }

        let mut encoder = renderer
            .state
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Mesh Skinning Compute Encoder"),
            });

        let shader = renderer.cache.compute_shader(HComputeShader::MESH_SKINNING);
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Mesh Skinning Compute Pass"),
                ..ComputePassDescriptor::default()
            });

            pass.set_pipeline(shader.pipeline());
            for (uniform, vertex_count) in self
                .skinning_uniforms
                .iter()
                .zip(self.skinning_vertex_counts.iter().copied())
            {
                if vertex_count == 0 {
                    continue;
                }
                pass.set_bind_group(0, uniform.bind_group(), &[]);
                pass.dispatch_workgroups(vertex_count.div_ceil(64), 1, 1);
            }
        }

        renderer.state.queue.submit(Some(encoder.finish()));
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

        let model_mat: glamx::Mat4 = (*local_to_world).into();
        data.mesh_data.update(&model_mat);

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

        let mut skinning_needs_dispatch = false;

        if self.bones_dirty {
            renderer.state.queue.write_buffer(
                data.uniform.buffer(MeshUniformIndex::BoneData),
                0,
                self.bone_data.as_bytes(),
            );
            self.bones_dirty = false;
            skinning_needs_dispatch = true;
        }

        if self.skinned && data.ensure_skinning_runtime(renderer, self.mesh) {
            skinning_needs_dispatch = true;
        }

        if self.skinned && skinning_needs_dispatch {
            data.dispatch_skinning(renderer);
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
        self.bounding
            .map(|b| b.transformed(&(*local_to_world).into()))
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

            let has_skinned_vertices = self.skinned
                && runtime.skinning_mesh == Some(self.mesh)
                && runtime.skinned_buffers.len() == mesh.meshlets().len();
            if has_skinned_vertices {
                mesh.draw_with_vertex_buffers(range.clone(), &runtime.skinned_buffers, pass);
            } else {
                mesh.draw(range.clone(), pass);
            }
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

        let mut data = RuntimeMeshData {
            mesh_data,
            uniform,
            skinned_buffers: Vec::new(),
            skinning_uniforms: Vec::new(),
            skinning_vertex_counts: Vec::new(),
            skinning_mesh: None,
        };

        if self.skinned {
            let _ = data.ensure_skinning_runtime(renderer, self.mesh);
            self.bones_dirty = true;
        }

        data
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
