use crate::gltf::loader::GltfImportErr;
use crate::gltf::loader::Result;
use gltf::Document;
use snafu::ResultExt;
use std::path::Path;

/// Container for a glTF document and its binary attachments.
pub struct GltfScene {
    pub doc: Document,
    pub buffers: Vec<gltf::buffer::Data>,
    pub images: Vec<gltf::image::Data>,
}

impl GltfScene {
    /// Imports a glTF scene from disk and gathers its buffers and images.
    pub fn import<P: AsRef<Path>>(path: P) -> Result<Self> {
        let (doc, buffers, images) = gltf::import(path).context(GltfImportErr)?;
        Ok(Self {
            doc,
            buffers,
            images,
        })
    }

    /// Imports a glTF scene from an in-memory byte slice.
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        let (doc, buffers, images) = gltf::import_slice(bytes).context(GltfImportErr)?;
        Ok(Self {
            doc,
            buffers,
            images,
        })
    }
}
