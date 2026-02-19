use crate::store::{H, HandleName, StoreType};
use glamx::{Quat, Vec3, Vec4};

#[derive(Debug, Clone, Default)]
pub struct PrefabAsset {
    pub source: String,
    pub root_nodes: Vec<u32>,
    pub nodes: Vec<PrefabNode>,
    pub animation_assets: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PrefabNode {
    pub name: String,
    pub local_position: Vec3,
    pub local_rotation: Quat,
    pub local_scale: Vec3,
    pub children: Vec<u32>,
    pub mesh: Option<PrefabMeshBinding>,
    pub extras_json: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PrefabMeshBinding {
    pub mesh_asset: String,
    pub material_hashes: Vec<Option<u64>>,
}

#[derive(Debug, Clone, Default)]
pub struct PrefabMaterial {
    pub name: String,
    pub base_color: Vec4,
    pub metallic: f32,
    pub roughness: f32,
    pub alpha_cutoff: Option<f32>,
    pub alpha_mode: String,
    pub double_sided: bool,
    pub unlit: bool,
    pub emissive_factor: Vec3,
    pub base_color_texture: Option<String>,
    pub normal_texture: Option<String>,
    pub metallic_roughness_texture: Option<String>,
    pub emissive_texture: Option<String>,
    pub occlusion_texture: Option<String>,
}

impl StoreType for PrefabMaterial {
    const NAME: &str = "PrefabMaterial";

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}

impl StoreType for PrefabAsset {
    const NAME: &str = "PrefabAsset";

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}
