use crate::cache::AssetCache;
use crate::cache::generic_cache::CacheType;
use crate::cache::render_mesh::PartialMeshCacheType;
use bitflags::bitflags;
use glamx::{Vec2, Vec3, Vec4};
use more_asserts::debug_assert_le;
use std::mem::size_of;
use std::ops::Range;
use std::sync::Arc;
use syrillian_asset::mesh::static_mesh_data::{RawSkinningVertexBuffers, RawVertexBuffers};
use syrillian_asset::mesh::{PartialMesh, SkinnedVertex3D};
use syrillian_asset::{Mesh, SkinnedMesh};
use syrillian_utils::debug_panic;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BufferUsages, Device, IndexFormat, Queue};
use zerocopy::{IntoBytes, Unalign};

#[derive(Debug, Clone)]
pub struct RenderVertexBuffers {
    pub position: wgpu::Buffer,
    pub uv: wgpu::Buffer,
    pub normal: wgpu::Buffer,
    pub tangent: wgpu::Buffer,
}

#[derive(Debug, Clone)]
pub struct PreSkinMeshlet {
    pub packed_input: wgpu::Buffer,
}

#[derive(Debug)]
pub struct Meshlet {
    pub vertex_buffers: RenderVertexBuffers,
    pub vertex_count: u32,
    pub index_buffer: Option<wgpu::Buffer>,
    pub index_count: u32,
    pub offset: u32,
    pub pre_skin: Option<PreSkinMeshlet>,
}

#[derive(Debug)]
pub struct RenderMesh {
    meshlets: Vec<Meshlet>,
    total_vertex_count: u32,
    total_index_count: u32,
}

bitflags! {
    #[derive(Debug, Copy, Clone)]
    pub struct BindMeshBuffers: u32 {
        const POSITION = 1;
        const UV = 1 << 1;
        const NORMAL = 1 << 2;
        const TANGENT = 1 << 3;
        const POSITION_NORMAL = Self::POSITION.bits() | Self::NORMAL.bits();
        const POSITION_UV = Self::POSITION.bits() | Self::UV.bits();
    }
}

impl RenderVertexBuffers {
    #[inline]
    fn create_buffer<T>(device: &Device, label: &'static str, vertex_count: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: size_of::<T>() as u64 * vertex_count,
            usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub fn new(device: &Device, vertex_count: u64) -> Self {
        let position = Self::create_buffer::<Vec3>(device, "Mesh Positions Buffer", vertex_count);
        let uv = Self::create_buffer::<Vec2>(device, "Mesh UV Buffer", vertex_count);
        let normal = Self::create_buffer::<Vec3>(device, "Mesh Normals Buffer", vertex_count);
        let tangent = Self::create_buffer::<Vec4>(device, "Mesh Tangents Buffer", vertex_count);

        Self {
            position,
            uv,
            normal,
            tangent,
        }
    }

    pub fn new_for_skinning(device: &Device, vertex_count: u64, uv: wgpu::Buffer) -> Self {
        let position = Self::create_buffer::<Vec3>(device, "Mesh Positions Buffer", vertex_count);
        let normal = Self::create_buffer::<Vec3>(device, "Mesh Normals Buffer", vertex_count);
        let tangent = Self::create_buffer::<Vec4>(device, "Mesh Tangents Buffer", vertex_count);

        Self {
            position,
            uv,
            normal,
            tangent,
        }
    }

    pub fn upload(raw: &RawVertexBuffers, device: &Device) -> Self {
        let position = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: raw.positions.as_bytes(),
            usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        let uv = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: raw.uvs.as_bytes(),
            usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        let normal = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: raw.normals.as_bytes(),
            usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        let tangent = device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: raw.tangents.as_bytes(),
            usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        Self {
            position,
            uv,
            normal,
            tangent,
        }
    }

    #[inline]
    fn bind(&self, pass: &mut wgpu::RenderPass<'_>, to_bind: BindMeshBuffers) {
        if to_bind.contains(BindMeshBuffers::POSITION) {
            pass.set_vertex_buffer(0, self.position.slice(..));
        }
        if to_bind.contains(BindMeshBuffers::UV) {
            pass.set_vertex_buffer(1, self.uv.slice(..));
        }
        if to_bind.contains(BindMeshBuffers::NORMAL) {
            pass.set_vertex_buffer(2, self.normal.slice(..));
        }
        if to_bind.contains(BindMeshBuffers::TANGENT) {
            pass.set_vertex_buffer(3, self.tangent.slice(..));
        }
    }
}

impl PreSkinMeshlet {
    pub fn upload(
        raw: &RawVertexBuffers,
        skinning: &RawSkinningVertexBuffers,
        range: Range<usize>,
        device: &Device,
    ) -> Self {
        debug_assert!(range.start <= range.end);
        debug_assert!(range.end <= raw.positions.len());
        debug_assert_eq!(raw.positions.len(), raw.uvs.len());
        debug_assert_eq!(raw.positions.len(), raw.normals.len());
        debug_assert_eq!(raw.positions.len(), raw.tangents.len());
        debug_assert_eq!(raw.positions.len(), skinning.bone_indices.len());
        debug_assert_eq!(raw.positions.len(), skinning.bone_weights.len());
        debug_assert_eq!(
            size_of::<SkinnedVertex3D>(),
            size_of::<u32>() * 18,
            "Packed skinning vertices must match shader WORDS_PER_VERTEX"
        );

        let packed_vertices: Vec<SkinnedVertex3D> = range
            .map(|idx| {
                let position = raw.positions[idx];
                let uv = raw.uvs[idx];
                let normal = raw.normals[idx];
                let tangent = raw.tangents[idx];
                let indices = skinning.bone_indices[idx];
                let weights = skinning.bone_weights[idx];

                SkinnedVertex3D {
                    position,
                    uv,
                    normal,
                    tangent: Unalign::new(tangent),
                    bone_indices: indices,
                    bone_weights: weights,
                }
            })
            .collect();

        let packed_input = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Mesh Packed Skinning Input Buffer"),
            contents: packed_vertices.as_bytes(),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        Self { packed_input }
    }
}

impl Meshlet {
    pub fn point_count(&self) -> u32 {
        if self.index_buffer.is_some() {
            self.index_count
        } else {
            self.vertex_count
        }
    }

    fn clamp_range(&self, range: Range<u32>, point_count: u32) -> Option<Range<u32>> {
        if !self.applies_to(range.clone(), point_count) {
            return None;
        }

        let start = range.start.saturating_sub(self.offset);
        let end = if range.end >= self.offset + point_count {
            point_count
        } else {
            range.end - self.offset
        };

        debug_assert_le!(start, end);
        debug_assert_le!(start, point_count);
        debug_assert_le!(end, point_count);

        Some(Range { start, end })
    }

    fn clamp_point_range(&self, range: Range<u32>) -> Option<Range<u32>> {
        self.clamp_range(range, self.point_count())
    }

    fn clamp_vertex_range(&self, range: Range<u32>) -> Option<Range<u32>> {
        self.clamp_range(range, self.vertex_count)
    }

    pub fn applies_to_points(&self, range: Range<u32>) -> bool {
        self.applies_to(range, self.point_count())
    }

    pub fn applies_to_vertices(&self, range: Range<u32>) -> bool {
        self.applies_to(range, self.vertex_count)
    }

    pub fn applies_to(&self, range: Range<u32>, point_count: u32) -> bool {
        self.offset < range.end && range.start <= self.offset + point_count
    }

    pub fn has_indices(&self) -> bool {
        self.index_buffer.is_some()
    }

    pub fn pre_skin_meshlet(&self) -> Option<&PreSkinMeshlet> {
        self.pre_skin.as_ref()
    }
}

impl Meshlet {
    pub fn bind(&self, pass: &mut wgpu::RenderPass<'_>, to_bind: BindMeshBuffers) {
        self.vertex_buffers.bind(pass, to_bind);
        if let Some(i_buffer) = &self.index_buffer {
            pass.set_index_buffer(i_buffer.slice(..), IndexFormat::Uint32);
        }
    }

    pub fn draw(
        &self,
        range: Range<u32>,
        pass: &mut wgpu::RenderPass<'_>,
        to_bind: BindMeshBuffers,
    ) {
        let Some(inner_range) = self.clamp_point_range(range) else {
            debug_panic!("Meshlet received invalid draw command");
            return;
        };

        self.bind(pass, to_bind);

        if self.has_indices() {
            pass.draw_indexed(inner_range, 0, 0..1);
        } else {
            pass.draw(inner_range, 0..1);
        }
    }

    pub fn draw_with_vertex_buffers(
        &self,
        range: Range<u32>,
        vertex_buffers: &RenderVertexBuffers,
        pass: &mut wgpu::RenderPass<'_>,
        bind_buffers: BindMeshBuffers,
    ) {
        let Some(inner_range) = self.clamp_point_range(range) else {
            debug_panic!("Meshlet received invalid draw command");
            return;
        };

        vertex_buffers.bind(pass, bind_buffers);

        if let Some(i_buffer) = &self.index_buffer {
            pass.set_index_buffer(i_buffer.slice(..), IndexFormat::Uint32);
            pass.draw_indexed(inner_range, 0, 0..1);
        } else {
            pass.draw(inner_range, 0..1);
        }
    }

    pub fn draw_as_instances(
        &self,
        vertices_instance_range: Range<u32>,
        vertices_range: Range<u32>,
        pass: &mut wgpu::RenderPass<'_>,
        bind_buffers: BindMeshBuffers,
    ) {
        // Instance data comes from the per-vertex buffers.
        let Some(inner_range) = self.clamp_vertex_range(vertices_instance_range) else {
            debug_panic!("Meshlet received invalid draw command");
            return;
        };

        self.bind(pass, bind_buffers);

        if self.has_indices() {
            pass.draw_indexed(vertices_range, 0, inner_range);
        } else {
            pass.draw(vertices_range, inner_range);
        }
    }
}

impl RenderMesh {
    pub fn new(meshlets: Vec<Meshlet>) -> Self {
        let mut mesh = Self {
            meshlets,
            total_vertex_count: 0,
            total_index_count: 0,
        };
        mesh.update_counts();
        mesh
    }

    pub fn set_meshlets(&mut self, meshlets: Vec<Meshlet>) {
        self.meshlets = meshlets;
        self.update_counts();
    }

    fn update_counts(&mut self) {
        self.total_vertex_count = 0;
        self.total_index_count = 0;

        for meshlet in &self.meshlets {
            debug_assert_eq!(meshlet.offset, self.total_vertex_count);

            self.total_vertex_count += meshlet.vertex_count;
            if meshlet.has_indices() {
                self.total_index_count += meshlet.index_count;
            }
        }
    }

    pub fn draw_all(&self, pass: &mut wgpu::RenderPass<'_>, bind_buffers: BindMeshBuffers) {
        self.draw(0..self.total_point_count(), pass, bind_buffers);
    }

    pub fn draw_all_as_instances(
        &self,
        vertices_range: Range<u32>,
        pass: &mut wgpu::RenderPass<'_>,
        bind_buffers: BindMeshBuffers,
    ) {
        self.draw_as_instances(
            0..self.total_vertex_count(),
            vertices_range,
            pass,
            bind_buffers,
        );
    }

    pub fn draw(
        &self,
        range: Range<u32>,
        pass: &mut wgpu::RenderPass<'_>,
        bind_buffers: BindMeshBuffers,
    ) {
        for meshlet in &self.meshlets {
            if range.end < meshlet.offset || range.start > meshlet.offset + meshlet.point_count() {
                continue;
            }

            meshlet.draw(range.clone(), pass, bind_buffers);
        }
    }

    pub fn draw_with_vertex_buffers(
        &self,
        range: Range<u32>,
        vertex_buffers: &[RenderVertexBuffers],
        pass: &mut wgpu::RenderPass<'_>,
        bind_buffers: BindMeshBuffers,
    ) {
        debug_assert_eq!(
            vertex_buffers.len(),
            self.meshlets.len(),
            "Skinned vertex buffers should match meshlet count"
        );

        for (meshlet, vertex_buffer) in self.meshlets.iter().zip(vertex_buffers) {
            if !meshlet.applies_to_points(range.clone()) {
                continue;
            }

            meshlet.draw_with_vertex_buffers(range.clone(), vertex_buffer, pass, bind_buffers);
        }
    }

    pub fn draw_as_instances(
        &self,
        vertices_instance_range: Range<u32>,
        vertices_range: Range<u32>,
        pass: &mut wgpu::RenderPass<'_>,
        bind_buffers: BindMeshBuffers,
    ) {
        for mesh in &self.meshlets {
            if !mesh.applies_to_vertices(vertices_instance_range.clone()) {
                continue;
            }

            mesh.draw_as_instances(
                vertices_instance_range.clone(),
                vertices_range.clone(),
                pass,
                bind_buffers,
            );
        }
    }

    pub fn meshlets(&self) -> &[Meshlet] {
        &self.meshlets
    }

    pub fn total_point_count(&self) -> u32 {
        if self.has_indices() {
            self.total_index_count
        } else {
            self.total_vertex_count
        }
    }

    #[inline]
    pub fn total_vertex_count(&self) -> u32 {
        self.total_vertex_count
    }

    #[inline]
    pub fn total_indices_count(&self) -> u32 {
        self.total_index_count
    }

    pub fn has_indices(&self) -> bool {
        self.total_indices_count() > 0
    }
}

impl PartialMeshCacheType for Mesh {}

fn upload_skinned_mesh(msg: SkinnedMesh, device: &Device) -> Arc<RenderMesh> {
    let max_buffer_size = device.limits().max_buffer_size as usize;

    let vertices_num = msg.position_count();
    let indices_num = msg.indices_count();
    let max_buffer_verts = max_buffer_size / size_of::<Vec4>();

    let mut meshlets = Vec::new();
    let raw = msg.buffers();
    let skinning = msg.skinning_data.as_ref();

    if let Some(indices) = msg.indices() {
        if vertices_num > max_buffer_verts {
            panic!(
                "FIXME: indexed mesh has more vertices than fit into one buffer without chunking"
            );
        }

        let max_buffer_indices = max_buffer_size / size_of::<u32>();
        let vertex_buffers = RenderVertexBuffers::upload(raw, device);
        let pre_skin = PreSkinMeshlet::upload(raw, skinning, 0..vertices_num, device);

        for i in 0..=(indices_num / max_buffer_indices) {
            let start = i * max_buffer_indices;
            let end = indices.len().min((i + 1) * max_buffer_indices);
            if start >= end {
                continue;
            }

            let indices_buf = device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Mesh Index Buffer"),
                contents: indices[start..end].as_bytes(),
                usage: BufferUsages::INDEX,
            });

            meshlets.push(Meshlet {
                vertex_buffers: vertex_buffers.clone(),
                vertex_count: vertices_num as u32,
                index_buffer: Some(indices_buf),
                index_count: (start..end).len() as u32,
                offset: start as u32,
                pre_skin: Some(pre_skin.clone()),
            })
        }
    } else {
        for i in 0..=(vertices_num / max_buffer_verts) {
            let start = i * max_buffer_verts;
            let end = vertices_num.min((i + 1) * max_buffer_verts);
            if start >= end {
                continue;
            }

            let vertex_buffers = RenderVertexBuffers::upload(raw, device);
            let pre_skin = PreSkinMeshlet::upload(raw, skinning, start..end, device);

            meshlets.push(Meshlet {
                vertex_buffers,
                vertex_count: (start..end).len() as u32,
                index_buffer: None,
                index_count: 0,
                offset: start as u32,
                pre_skin: Some(pre_skin),
            })
        }
    }

    Arc::new(RenderMesh::new(meshlets))
}

impl CacheType for SkinnedMesh {
    type Hot = Arc<RenderMesh>;
    type UpdateMessage = Self;

    fn upload(
        msg: Self::UpdateMessage,
        device: &Device,
        _queue: &Queue,
        _cache: &AssetCache,
    ) -> Self::Hot {
        upload_skinned_mesh(msg, device)
    }
}

impl CacheType for Mesh {
    type Hot = Arc<RenderMesh>;
    type UpdateMessage = Self;

    fn upload(
        msg: Self::UpdateMessage,
        device: &Device,
        _queue: &Queue,
        _cache: &AssetCache,
    ) -> Self::Hot {
        msg.upload(device)
    }
}
