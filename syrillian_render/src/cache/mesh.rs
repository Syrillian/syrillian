use crate::cache::AssetCache;
use crate::cache::generic_cache::CacheType;
use more_asserts::debug_assert_le;
use std::ops::Range;
use std::sync::Arc;
use syrillian_asset::Mesh;
use syrillian_asset::mesh::Vertex3D;
use syrillian_utils::debug_panic;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BufferUsages, Device, IndexFormat, Queue};

#[derive(Debug)]
pub struct Meshlet {
    pub vertex_buffer: wgpu::Buffer,
    pub vertex_count: u32,
    pub index_buffer: Option<wgpu::Buffer>,
    pub index_count: u32,
    pub offset: u32,
}

#[derive(Debug)]
pub struct RuntimeMesh {
    meshlets: Vec<Meshlet>,
    total_vertex_count: u32,
    total_index_count: u32,
}

impl Meshlet {
    pub fn point_count(&self) -> u32 {
        if self.index_buffer.is_some() {
            self.index_count
        } else {
            self.vertex_count
        }
    }

    pub fn bind(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        if let Some(i_buffer) = &self.index_buffer {
            pass.set_index_buffer(i_buffer.slice(..), IndexFormat::Uint32);
        }
    }

    pub fn draw(&self, range: Range<u32>, pass: &mut wgpu::RenderPass<'_>) {
        let Some(inner_range) = self.clamp_range(range) else {
            debug_panic!("Meshlet received invalid draw command");
            return;
        };

        self.bind(pass);

        if self.has_indices() {
            pass.draw_indexed(inner_range, 0, 0..1);
        } else {
            pass.draw(inner_range, 0..1);
        }
    }

    pub fn draw_as_instances(
        &self,
        range: Range<u32>,
        vertices_range: Range<u32>,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        let Some(inner_range) = self.clamp_range(range) else {
            debug_panic!("Meshlet received invalid draw command");
            return;
        };

        self.bind(pass);

        if self.has_indices() {
            pass.draw_indexed(vertices_range, 0, inner_range);
        } else {
            pass.draw(vertices_range, inner_range);
        }
    }

    fn clamp_range(&self, range: Range<u32>) -> Option<Range<u32>> {
        if !self.applies_to(range.clone()) {
            return None;
        }

        let start = range.start.saturating_sub(self.offset);
        let end = if range.end >= self.offset + self.point_count() {
            self.point_count()
        } else {
            range.end - self.offset
        };

        debug_assert_le!(start, end);
        debug_assert_le!(start, self.point_count());
        debug_assert_le!(end, self.point_count());

        Some(Range { start, end })
    }

    pub fn applies_to(&self, range: Range<u32>) -> bool {
        self.offset < range.end && range.start <= self.offset + self.point_count()
    }

    pub fn has_indices(&self) -> bool {
        self.index_buffer.is_some()
    }
}

impl RuntimeMesh {
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
    }

    fn update_counts(&mut self) {
        self.total_index_count = 0;
        self.total_index_count = 0;

        for meshlet in &self.meshlets {
            debug_assert_eq!(meshlet.offset, self.total_vertex_count);

            self.total_vertex_count += meshlet.vertex_count;
            if meshlet.has_indices() {
                self.total_index_count += meshlet.index_count;
            }
        }
    }

    pub fn draw_all(&self, pass: &mut wgpu::RenderPass<'_>) {
        self.draw(0..self.total_point_count(), pass);
    }

    pub fn draw_all_as_instances(
        &self,
        vertices_range: Range<u32>,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        self.draw_as_instances(0..self.total_point_count(), vertices_range, pass);
    }

    pub fn draw(&self, range: Range<u32>, pass: &mut wgpu::RenderPass<'_>) {
        // TODO: Check that meshlets are ordered so that iteration can end when the range passes a meshlet
        for mesh in &self.meshlets {
            if range.end < mesh.offset || range.start > mesh.offset + mesh.point_count() {
                continue;
            }

            mesh.draw(range.clone(), pass);
        }
    }

    pub fn draw_as_instances(
        &self,
        range: Range<u32>,
        vertices_range: Range<u32>,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        for mesh in &self.meshlets {
            if !mesh.applies_to(range.clone()) {
                continue;
            }

            mesh.draw_as_instances(range.clone(), vertices_range.clone(), pass);
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

const MAX_BUFFER_VERTS: usize = 128_000_000 / size_of::<Vertex3D>(); // 128MiB limit
const MAX_BUFFER_INDICES: usize = 128_000_000 / size_of::<u32>(); // 128MiB limit

impl CacheType for Mesh {
    type Hot = Arc<RuntimeMesh>;

    fn upload(self, device: &Device, _queue: &Queue, _cache: &AssetCache) -> Self::Hot {
        let vertices_num = self.vertex_count();
        let indices_num = self.indices_count();

        let mut meshlets = Vec::new();

        // TODO: Chunk indexed meshes properly
        if let Some(indices) = self.indices() {
            if vertices_num > MAX_BUFFER_VERTS {
                panic!(
                    "FIXME: indexed mesh has more vertices than fit into one buffer without chunking"
                );
            }

            let vertex_buf = device.create_buffer_init(&BufferInitDescriptor {
                label: Some("Mesh Vertex Buffer"),
                contents: bytemuck::cast_slice(self.vertices()),
                usage: BufferUsages::VERTEX,
            });

            for i in 0..=(indices_num / MAX_BUFFER_INDICES) {
                let start = i * MAX_BUFFER_INDICES;
                let end = indices.len().min((i + 1) * MAX_BUFFER_INDICES);
                let indices_buf = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("Mesh Index Buffer"),
                    contents: bytemuck::cast_slice(&indices[start..end]),
                    usage: BufferUsages::INDEX,
                });
                meshlets.push(Meshlet {
                    vertex_buffer: vertex_buf.clone(),
                    vertex_count: vertices_num as u32,
                    index_buffer: Some(indices_buf),
                    index_count: (start..end).len() as u32,
                    offset: start as u32,
                })
            }
        } else {
            let vertices = self.vertices();
            for i in 0..=(vertices_num / MAX_BUFFER_VERTS) {
                let start = i * MAX_BUFFER_VERTS;
                let end = vertices.len().min((i + 1) * MAX_BUFFER_VERTS);
                let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("Mesh Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices[start..end]),
                    usage: BufferUsages::VERTEX,
                });
                meshlets.push(Meshlet {
                    vertex_buffer,
                    vertex_count: (start..end).len() as u32,
                    index_buffer: None,
                    index_count: 0,
                    offset: start as u32,
                })
            }
        }

        Arc::new(RuntimeMesh::new(meshlets))
    }
}
