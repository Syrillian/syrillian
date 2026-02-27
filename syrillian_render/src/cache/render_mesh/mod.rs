use crate::cache::mesh::{Meshlet, RenderVertexBuffers};
use crate::rendering::mesh::RenderMesh;
use glamx::Vec4;
use std::sync::Arc;
use syrillian_asset::mesh::PartialMesh;
use syrillian_asset::store::StoreType;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{BufferUsages, Device};
use zerocopy::IntoBytes;

pub mod mesh;

pub trait PartialMeshCacheType: PartialMesh + StoreType {
    #[profiling::function]
    fn upload(self, device: &Device) -> Arc<RenderMesh> {
        let max_buffer_size = device.limits().max_buffer_size as usize;

        let vertices_num = self.position_count();
        let indices_num = self.indices_count();
        let max_buffer_verts = max_buffer_size / size_of::<Vec4>();

        let mut meshlets = Vec::new();

        if let Some(indices) = self.indices() {
            if vertices_num > max_buffer_verts {
                panic!(
                    "FIXME: indexed mesh has more vertices than fit into one buffer without chunking"
                );
            }
            let max_buffer_indices = max_buffer_size / size_of::<u32>();

            let vertex_buffers = RenderVertexBuffers::upload(self.buffers(), device);

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
                    pre_skin: None,
                })
            }
        } else {
            for i in 0..=(vertices_num / max_buffer_verts) {
                let start = i * max_buffer_verts;
                let end = vertices_num.min((i + 1) * max_buffer_verts);
                if start >= end {
                    continue;
                }

                let vertex_buffers = RenderVertexBuffers::upload(self.buffers(), device);

                meshlets.push(Meshlet {
                    vertex_buffers,
                    vertex_count: (start..end).len() as u32,
                    index_buffer: None,
                    index_count: 0,
                    offset: start as u32,
                    pre_skin: None,
                })
            }
        }

        Arc::new(RenderMesh::new(meshlets))
    }
}
