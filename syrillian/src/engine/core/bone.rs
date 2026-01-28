use crate::math::Mat4;
use crate::{MAX_BONES, ensure_aligned};
use std::collections::HashMap;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Bone {
    pub(crate) transform: Mat4,
}

ensure_aligned!(Bone { transform }, align <= 16 * 4 => size);

#[derive(Debug, Default, Clone)]
pub struct Bones {
    /// Index-aligned bone names.
    pub names: Vec<String>,
    /// Parent bone index; None for roots.
    pub parents: Vec<Option<usize>>,
    pub children: Vec<Vec<usize>>,
    pub roots: Vec<usize>,
    pub inverse_bind: Vec<Mat4>,
    pub bind_global: Vec<Mat4>,
    pub bind_local: Vec<Mat4>,
    /// Fast lookup from name to index.
    pub index_of: HashMap<String, usize>,
}

impl Bones {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    pub fn index(&self, name: &str) -> Option<usize> {
        self.index_of.get(name).copied()
    }

    pub fn as_slice(&self) -> &[Mat4] {
        self.inverse_bind.as_slice()
    }

    pub fn none() -> Bones {
        Bones::default()
    }
}

#[derive(Debug, Default, Clone)]
pub struct BoneData {
    pub(crate) bones: Vec<Bone>,
}

impl BoneData {
    #[rustfmt::skip]
    pub const DUMMY: [Bone; MAX_BONES] = [Bone {
        transform: Mat4::IDENTITY,
    }; MAX_BONES];

    pub fn new_full_identity() -> Self {
        Self {
            bones: vec![
                Bone {
                    transform: Mat4::IDENTITY
                };
                MAX_BONES
            ],
        }
    }

    pub fn set_first_n(&mut self, mats: &[Mat4]) {
        for (i, m) in mats.iter().take(self.bones.len()).enumerate() {
            self.bones[i].transform = *m;
        }
    }

    pub fn count(&self) -> usize {
        self.bones.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.bones)
    }
}
