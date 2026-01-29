use crate::assets::{AssetStore, HMesh, HShader};
use crate::core::ModelUniform;
use crate::core::bone::BoneData;
use crate::math::{Vec3, Vec4};
use crate::rendering::proxies::mesh_proxy::{MeshUniformIndex, RuntimeMeshData};
use crate::rendering::proxies::{PROXY_PRIORITY_SOLID, SceneProxy, SceneProxyBinding};
use crate::rendering::uniform::ShaderUniform;
use crate::rendering::{AssetCache, GPUDrawCtx, Renderer};
use crate::{must_pipeline, proxy_data, proxy_data_mut, try_activate_shader};
use glamx::Affine3A;
use std::any::Any;
use syrillian_utils::debug_panic;
use tracing::warn;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{Buffer, BufferUsages, Device, Queue};

#[derive(Debug)]
pub(crate) struct GPUDebugProxyData {
    line_data: Option<Buffer>,
    model_uniform: Option<RuntimeMeshData>,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct DebugLine {
    pub start: Vec3,
    pub start_color: Vec4,
    pub end: Vec3,
    pub end_color: Vec4,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DebugLineVertex {
    position: [f32; 3],
    color: [f32; 4],
}

impl DebugLineVertex {
    fn new(position: Vec3, color: Vec4) -> Self {
        Self {
            position: [position.x, position.y, position.z],
            color: [color.x, color.y, color.z, color.w],
        }
    }
}

#[derive(Debug)]
pub struct DebugSceneProxy {
    pub lines: Vec<DebugLine>,
    pub meshes: Vec<HMesh>,
    pub color: Vec4,
    pub override_transform: Option<Affine3A>,
}

impl SceneProxy for DebugSceneProxy {
    fn setup_render(&mut self, renderer: &Renderer, model_mat: &Affine3A) -> Box<dyn Any> {
        let line_data = self.new_line_buffer(&renderer.state.device);
        let transform = self.override_transform.unwrap_or(*model_mat);
        let model_uniform =
            self.new_mesh_buffer(&renderer.cache, &renderer.state.device, &transform);

        Box::new(GPUDebugProxyData {
            line_data,
            model_uniform,
        })
    }

    fn update_render(
        &mut self,
        renderer: &Renderer,
        data: &mut dyn Any,
        local_to_world: &Affine3A,
    ) {
        let data: &mut GPUDebugProxyData = proxy_data_mut!(data);

        // TODO: Reuse or Resize buffer
        data.line_data = self.new_line_buffer(&renderer.state.device);

        let transform = self.override_transform.unwrap_or(*local_to_world);
        self.update_mesh_buffer(
            data,
            &renderer.cache,
            &renderer.state.device,
            &renderer.state.queue,
            &transform,
        );
    }

    fn render(&self, renderer: &Renderer, ctx: &GPUDrawCtx, binding: &SceneProxyBinding) {
        let data = proxy_data!(binding.proxy_data());
        let cache = &renderer.cache;
        self.render_lines(data, cache, ctx);
        self.render_meshes(data, cache, ctx)
    }

    fn priority(&self, _store: &AssetStore) -> u32 {
        PROXY_PRIORITY_SOLID
    }
}

impl Default for DebugSceneProxy {
    fn default() -> Self {
        Self {
            lines: vec![],
            meshes: vec![],
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            override_transform: None,
        }
    }
}

impl DebugSceneProxy {
    fn new_line_buffer(&self, device: &Device) -> Option<Buffer> {
        if self.lines.is_empty() {
            return None;
        }

        let mut vertices = Vec::with_capacity(self.lines.len() * 2);
        for line in &self.lines {
            vertices.push(DebugLineVertex::new(line.start, line.start_color));
            vertices.push(DebugLineVertex::new(line.end, line.end_color));
        }

        Some(device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Debug Ray Data Buffer"),
            contents: bytemuck::cast_slice(vertices.as_slice()),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        }))
    }

    fn new_mesh_buffer(
        &self,
        cache: &AssetCache,
        device: &Device,
        model_mat: &Affine3A,
    ) -> Option<RuntimeMeshData> {
        if self.meshes.is_empty() {
            return None;
        }

        let bgl = cache.bgl_model();
        let mesh_data = ModelUniform {
            model_mat: (*model_mat).into(),
        };
        let uniform = ShaderUniform::builder((*bgl).clone())
            .with_buffer_data(&mesh_data)
            .with_buffer_data(&BoneData::DUMMY)
            .build(device);

        Some(RuntimeMeshData { mesh_data, uniform })
    }

    fn update_mesh_buffer(
        &self,
        data: &mut GPUDebugProxyData,
        cache: &AssetCache,
        device: &Device,
        queue: &Queue,
        model_mat: &Affine3A,
    ) {
        if self.meshes.is_empty() {
            return;
        }

        let model_uniform = match data.model_uniform.take() {
            None => self.new_mesh_buffer(cache, device, model_mat),
            Some(mut model_uniform) => {
                model_uniform.mesh_data.model_mat = (*model_mat).into();
                let mesh_buffer = model_uniform.uniform.buffer(MeshUniformIndex::MeshData);
                queue.write_buffer(mesh_buffer, 0, bytemuck::bytes_of(&model_uniform.mesh_data));
                Some(model_uniform)
            }
        };

        data.model_uniform = model_uniform;
    }

    pub fn single_mesh(mesh: HMesh) -> Self {
        let mut proxy = Self::default();
        proxy.meshes.push(mesh);
        proxy
    }

    pub fn set_override_transform(&mut self, transform: Affine3A) {
        self.override_transform = Some(transform);
    }

    fn render_lines(&self, data: &GPUDebugProxyData, cache: &AssetCache, ctx: &GPUDrawCtx) {
        if self.lines.is_empty() {
            return;
        }

        let Some(line_buffer) = &data.line_data else {
            debug_panic!("Lines exist but line buffer was not prepared when rendering.");
            return;
        };

        let mut pass = ctx.pass.write().unwrap();
        let shader = cache.shader(HShader::DEBUG_LINES);
        try_activate_shader!(shader, &mut pass, ctx => return);

        pass.set_vertex_buffer(0, line_buffer.slice(..));
        let vertices = self.lines.len() as u32 * 2;
        pass.draw(0..vertices, 0..1);
    }

    fn render_meshes(&self, data: &GPUDebugProxyData, cache: &AssetCache, ctx: &GPUDrawCtx) {
        if self.meshes.is_empty() {
            return;
        }

        let Some(data) = &data.model_uniform else {
            debug_panic!("Meshes exist but mesh buffer was not prepared when rendering.");
            return;
        };

        for mesh in self.meshes.iter().copied() {
            let Some(runtime_mesh) = cache.meshes.try_get(mesh, cache) else {
                warn!("Couldn't render {}", mesh.ident_fmt());
                continue;
            };

            let shader = cache.shader(HShader::DEBUG_EDGES);
            let groups = shader.bind_groups();
            must_pipeline!(pipeline = shader, ctx.pass_type => return);

            let mut pass = ctx.pass.write().unwrap();

            pass.set_pipeline(pipeline);
            pass.set_immediates(0, bytemuck::bytes_of(&self.color));
            pass.set_bind_group(groups.render, ctx.render_bind_group, &[]);
            if let Some(idx) = groups.model {
                pass.set_bind_group(idx, data.uniform.bind_group(), &[]);
            }

            runtime_mesh.draw_all(&mut pass);
        }
    }
}
