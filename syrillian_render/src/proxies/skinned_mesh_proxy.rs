// TODO: refactor

use crate::cache::mesh::{RenderMesh, RenderVertexBuffers};
use crate::cache::{AssetCache, RuntimeShader};
use crate::model_uniform::ModelUniform;
use crate::proxies::{
    MeshUniformIndex, PROXY_PRIORITY_SOLID, PROXY_PRIORITY_TRANSPARENT, SceneProxy,
    SceneProxyBinding,
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
use static_assertions::const_assert_eq;
use std::any::Any;
use std::ops::Range;
use syrillian_asset::mesh::bone::BoneData;
use syrillian_asset::store::H;
use syrillian_asset::{HComputeShader, HMaterialInstance, HSkinnedMesh, Shader};
use syrillian_macros::UniformIndex;
use syrillian_render::rendering::mesh::BindMeshBuffers;
use syrillian_utils::BoundingSphere;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor, RenderPass};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

#[repr(u8)]
#[derive(Copy, Clone, Debug, UniformIndex)]
pub enum SkinnedMeshUniformIndex {
    MeshData = 0,
    BoneData = 1,
}

#[derive(Debug, Clone)]
pub struct RenderSkinnedMeshData {
    pub mesh_data: ModelUniform,
    // TODO: Consider having a uniform like that, for every Transform by default in some way, or
    //       lazy-make / provide one by default.
    pub mesh_uniform: ShaderUniform<MeshUniformIndex>,
    pub skinning_uniform: ShaderUniform<SkinnedMeshUniformIndex>,
    pub skinned_meshlets: Vec<RenderVertexBuffers>,
    pub skinning_uniforms: Vec<ShaderUniform<MeshSkinningComputeUniformIndex>>,
    pub skinning_vertex_counts: Vec<u32>,
    pub skinning_mesh: Option<HSkinnedMesh>,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Immutable, IntoBytes, FromBytes, KnownLayout)]
struct MeshSkinningParams {
    vertex_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

const_assert_eq!(size_of::<MeshSkinningParams>(), 16);

#[repr(u8)]
#[derive(Copy, Clone, Debug, UniformIndex)]
pub enum MeshSkinningComputeUniformIndex {
    Bones = 0,
    Params = 1,
    SourcePacked = 2,
    DestPosition = 3,
    DestNormal = 4,
    DestTangent = 5,
}

#[derive(Debug, Clone)]
pub struct SkinnedMeshSceneProxy {
    pub mesh: HSkinnedMesh,
    pub materials: Vec<HMaterialInstance>,
    pub material_ranges: Vec<Range<u32>>,
    pub bone_data: BoneData,
    pub bones_dirty: bool,
    pub bounding: Option<BoundingSphere>,
}

impl RenderSkinnedMeshData {
    pub fn activate_shader(
        &self,
        shader: &RuntimeShader,
        ctx: &GPUDrawCtx,
        pass: &mut RenderPass,
    ) -> bool {
        try_activate_shader!(shader, pass, ctx => return false);

        if let Some(idx) = shader.bind_groups().model {
            pass.set_bind_group(idx, self.mesh_uniform.bind_group(), &[]);
        }

        true
    }

    fn ensure_skinning_runtime(&mut self, renderer: &Renderer, mesh_handle: HSkinnedMesh) -> bool {
        let valid_existing = self.skinning_mesh == Some(mesh_handle)
            && self.skinned_meshlets.len() == self.skinning_uniforms.len()
            && self.skinned_meshlets.len() == self.skinning_vertex_counts.len()
            && !self.skinned_meshlets.is_empty();
        if valid_existing {
            return false;
        }

        self.skinned_meshlets.clear();
        self.skinning_uniforms.clear();
        self.skinning_vertex_counts.clear();
        self.skinning_mesh = Some(mesh_handle);

        let Some(mesh) = renderer.cache.skinned_mesh(mesh_handle) else {
            return false;
        };

        let device = &renderer.state.device;
        let skinning_bgl = renderer.cache.bgl_mesh_skinning_compute();
        let bone_buffer = self
            .skinning_uniform
            .buffer(SkinnedMeshUniformIndex::BoneData)
            .clone();

        for meshlet in mesh.meshlets() {
            let vertex_count = meshlet.vertex_count;
            let Some(pre_skin) = meshlet.pre_skin_meshlet() else {
                self.skinned_meshlets.clear();
                self.skinning_uniforms.clear();
                self.skinning_vertex_counts.clear();
                self.skinning_mesh = None;
                return false;
            };

            let output = RenderVertexBuffers::new_for_skinning(
                device,
                vertex_count as u64,
                meshlet.vertex_buffers.uv.clone(),
            );
            let params = MeshSkinningParams {
                vertex_count,
                _pad0: 0,
                _pad1: 0,
                _pad2: 0,
            };

            let uniform =
                ShaderUniform::<MeshSkinningComputeUniformIndex>::builder(skinning_bgl.clone())
                    .with_buffer(bone_buffer.clone())
                    .with_buffer_data(&params)
                    .with_storage_buffer(pre_skin.packed_input.clone())
                    .with_storage_buffer(output.position.clone())
                    .with_storage_buffer(output.normal.clone())
                    .with_storage_buffer(output.tangent.clone())
                    .build(device);

            self.skinned_meshlets.push(output);
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

impl SceneProxy for SkinnedMeshSceneProxy {
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
        let data: &mut RenderSkinnedMeshData = proxy_data_mut!(data);

        let model_mat: glamx::Mat4 = (*local_to_world).into();
        data.mesh_data.update(&model_mat);

        renderer.state.queue.write_buffer(
            data.skinning_uniform
                .buffer(SkinnedMeshUniformIndex::MeshData),
            0,
            data.mesh_data.as_bytes(),
        );
    }

    fn update_render(
        &mut self,
        renderer: &Renderer,
        data: &mut (dyn Any + Send),
        _local_to_world: &Affine3A,
    ) {
        let data: &mut RenderSkinnedMeshData = proxy_data_mut!(data);

        // TODO: Consider Rigid Body render isometry interpolation for mesh local to world

        let mut skinning_needs_dispatch = false;

        if self.bones_dirty {
            renderer.state.queue.write_buffer(
                data.skinning_uniform
                    .buffer(SkinnedMeshUniformIndex::BoneData),
                0,
                self.bone_data.as_bytes(),
            );
            self.bones_dirty = false;
            skinning_needs_dispatch = true;
        }

        if data.ensure_skinning_runtime(renderer, self.mesh) {
            skinning_needs_dispatch = true;
        }

        if skinning_needs_dispatch {
            data.dispatch_skinning(renderer);
        }
    }

    fn render<'a>(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        let data: &RenderSkinnedMeshData = proxy_data!(binding.proxy_data());

        let Some(mesh) = renderer.cache.skinned_mesh(self.mesh) else {
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
        let data: &RenderSkinnedMeshData = proxy_data!(binding.proxy_data());

        let Some(mesh) = renderer.cache.skinned_mesh(self.mesh) else {
            return;
        };

        let mut pass = ctx.pass.write();
        self.draw_mesh_shadow(ctx, &renderer.cache, &mesh, data, &mut pass);
    }

    // TODO: Make shaders more modular so picking and (shadow) shaders can be generated from just a vertex shader
    fn render_picking(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        debug_assert_ne!(ctx.pass_type, RenderPassType::Shadow);

        let data: &RenderSkinnedMeshData = proxy_data!(binding.proxy_data());

        let Some(mesh) = renderer.cache.skinned_mesh(self.mesh) else {
            return;
        };

        let mut pass = ctx.pass.write();

        let color = hash_to_rgba(binding.object_hash);
        pass.set_immediates(0, color.as_bytes());

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

    fn bounds(&self, local_to_world: &Affine3A) -> Option<BoundingSphere> {
        self.bounding
            .map(|b| b.transformed(&(*local_to_world).into()))
    }
}

impl SkinnedMeshSceneProxy {
    fn draw_mesh_base(
        &self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        mesh: &RenderMesh,
        runtime: &RenderSkinnedMeshData,
        pass: &mut RwLockWriteGuard<RenderPass>,
    ) {
        self.draw_materials(ctx, cache, mesh, runtime, pass, RenderPassType::Color);
    }

    fn draw_mesh_shadow(
        &self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        mesh: &RenderMesh,
        runtime: &RenderSkinnedMeshData,
        pass: &mut RwLockWriteGuard<RenderPass>,
    ) {
        self.draw_materials(ctx, cache, mesh, runtime, pass, RenderPassType::Shadow);
    }

    fn draw_mesh_picking(
        &self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        mesh: &RenderMesh,
        runtime: &RenderSkinnedMeshData,
        pass: &mut RwLockWriteGuard<RenderPass>,
    ) {
        self.draw_materials(ctx, cache, mesh, runtime, pass, RenderPassType::Picking);
    }

    fn draw_materials(
        &self,
        ctx: &GPUDrawCtx,
        cache: &AssetCache,
        mesh: &RenderMesh,
        runtime: &RenderSkinnedMeshData,
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

            debug_assert_eq!(runtime.skinned_meshlets.len(), mesh.meshlets().len());

            mesh.draw_with_vertex_buffers(
                range.clone(),
                &runtime.skinned_meshlets,
                pass,
                BindMeshBuffers::all(),
            );
        }
    }

    fn setup_mesh_data(
        &mut self,
        renderer: &Renderer,
        local_to_world: &Affine3A,
    ) -> RenderSkinnedMeshData {
        let device = &renderer.state.device;
        let model_bgl = renderer.cache.bgl_model();
        let skinning_model_bgl = renderer.cache.bgl_model_skinning();
        let mesh_data = ModelUniform::from_matrix(&(*local_to_world).into());

        let mesh_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Skinned Mesh Buffer"),
            contents: mesh_data.as_bytes(),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let mesh_uniform = ShaderUniform::<MeshUniformIndex>::builder(model_bgl)
            .with_buffer(mesh_buffer.clone())
            .build(device);

        let skinning_uniform =
            ShaderUniform::<SkinnedMeshUniformIndex>::builder(skinning_model_bgl)
                .with_buffer(mesh_buffer)
                .with_buffer_data_slice(self.bone_data.bones.as_slice())
                .build(device);

        let mut data = RenderSkinnedMeshData {
            mesh_data,
            mesh_uniform,
            skinning_uniform,
            skinned_meshlets: Vec::new(),
            skinning_uniforms: Vec::new(),
            skinning_vertex_counts: Vec::new(),
            skinning_mesh: None,
        };

        self.bones_dirty = data.ensure_skinning_runtime(renderer, self.mesh);

        data
    }
}

#[cfg(debug_assertions)]
fn draw_edges(
    ctx: &GPUDrawCtx,
    cache: &AssetCache,
    mesh: &RenderMesh,
    runtime: &RenderSkinnedMeshData,
    pass: &mut RenderPass,
) {
    use glamx::Vec4;
    use syrillian_asset::HShader;

    const COLOR: Vec4 = Vec4::new(1.0, 0.0, 1.0, 1.0);

    let shader = cache.shader(HShader::DEBUG_EDGES);
    if !runtime.activate_shader(&shader, ctx, pass) {
        return;
    }

    pass.set_immediates(0, COLOR.as_bytes());

    mesh.draw_all(pass, BindMeshBuffers::POSITION);
}

#[cfg(debug_assertions)]
fn draw_vertex_normals(
    ctx: &GPUDrawCtx,
    cache: &AssetCache,
    mesh: &RenderMesh,
    runtime: &RenderSkinnedMeshData,
    pass: &mut RenderPass,
) {
    use syrillian_asset::HShader;

    let shader = cache.shader(HShader::DEBUG_VERTEX_NORMALS);
    if !runtime.activate_shader(&shader, ctx, pass) {
        return;
    }

    mesh.draw_all_as_instances(0..2, pass, BindMeshBuffers::POSITION_NORMAL);
}
