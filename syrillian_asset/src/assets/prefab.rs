use crate::store::streaming::asset_store::{AssetType, StreamingAssetFile, StreamingAssetPayload};
use crate::store::streaming::decode_helper::{DecodeHelper, MapDecodeHelper, ParseDecode};
use crate::store::streaming::packaged_scene::BuiltPayload;
use crate::store::streaming::payload::StreamableAsset;
use crate::store::{AssetKey, AssetRefreshMessage, H, HandleName, StoreType, streaming};
use crossbeam_channel::Sender;
use glamx::{Quat, Vec3, Vec4};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
use syrillian_reflect::Value;
use syrillian_reflect::serializer::JsonSerializer;

#[derive(Debug, Clone, Default)]
pub struct PrefabAsset {
    pub source: String,
    pub root_nodes: Vec<u32>,
    pub nodes: Vec<PrefabNode>,
    pub animation_assets: Vec<String>,
}

/// A serialized component attached to a prefab node.
/// Only reflected fields are stored; non-reflected fields use Default values on load.
#[derive(Debug, Clone, Default)]
pub struct PrefabComponent {
    pub type_name: String,
    pub fields: BTreeMap<String, Value>,
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
    pub components: Vec<PrefabComponent>,
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
    const TYPE: AssetType = AssetType::Prefab;

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn refresh_dirty(&self, _key: AssetKey, _assets_tx: &Sender<AssetRefreshMessage>) -> bool {
        false
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}

impl StoreType for PrefabAsset {
    const NAME: &str = "PrefabAsset";
    const TYPE: AssetType = AssetType::Prefab;

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn refresh_dirty(&self, _key: AssetKey, _assets_tx: &Sender<AssetRefreshMessage>) -> bool {
        false
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}

fn json_to_reflect_value(json: &JsonValue) -> Value {
    match json {
        JsonValue::Null => Value::None,
        JsonValue::Bool(b) => Value::Bool(*b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::BigInt(i)
            } else if let Some(u) = n.as_u64() {
                Value::BigUInt(u)
            } else {
                Value::Double(n.as_f64().unwrap_or(0.0))
            }
        }
        JsonValue::String(s) => Value::String(s.clone()),
        JsonValue::Array(arr) => Value::Array(arr.iter().map(json_to_reflect_value).collect()),
        JsonValue::Object(obj) => Value::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), json_to_reflect_value(v)))
                .collect(),
        ),
    }
}

fn json_object_to_value_map(json: &JsonValue) -> BTreeMap<String, Value> {
    match json {
        JsonValue::Object(obj) => obj
            .iter()
            .map(|(k, v)| (k.clone(), json_to_reflect_value(v)))
            .collect(),
        _ => BTreeMap::new(),
    }
}

impl ParseDecode<PrefabNode> for JsonValue {
    fn expect_parse(&self, _label: &str) -> streaming::error::Result<PrefabNode> {
        let object = self.expect_object("prefab node")?;

        let mesh = match object.optional_field("mesh") {
            None => None,
            Some(JsonValue::Null) => None,
            Some(mesh_value) => {
                let mesh = mesh_value.expect_object("prefab mesh binding")?;
                Some(PrefabMeshBinding {
                    mesh_asset: mesh
                        .required_field("mesh_asset")?
                        .expect_parse("prefab mesh asset path")?,
                    material_hashes: mesh
                        .required_field("material_hashes")?
                        .expect_parse("prefab material hashes")?,
                })
            }
        };

        let extras_json = match object.optional_field("extras_json") {
            None | Some(JsonValue::Null) => None,
            Some(JsonValue::String(value)) => Some(value.clone()),
            Some(value) => Some(value.to_string()),
        };

        let components = match object.optional_field("components") {
            None | Some(JsonValue::Null) => Vec::new(),
            Some(JsonValue::Array(arr)) => arr
                .iter()
                .filter_map(|item| {
                    let obj = item.as_object()?;
                    let type_name = obj.get("type_name")?.as_str()?.to_string();
                    let fields_value = obj.get("fields")?;
                    let fields = json_object_to_value_map(fields_value);
                    Some(PrefabComponent { type_name, fields })
                })
                .collect(),
            _ => Vec::new(),
        };

        Ok(PrefabNode {
            name: object
                .required_field("name")?
                .expect_parse("prefab node name")?,
            local_position: object
                .required_field("local_position")?
                .expect_parse("prefab local_position")?,
            local_rotation: object
                .required_field("local_rotation")?
                .expect_parse("prefab local_rotation")?,
            local_scale: object
                .required_field("local_scale")?
                .expect_parse("prefab local_scale")?,
            children: object
                .required_field("children")?
                .expect_parse("prefab children")?,
            mesh,
            extras_json,
            components,
        })
    }
}

impl StreamableAsset for PrefabAsset {
    fn encode(&self) -> BuiltPayload {
        BuiltPayload {
            payload: JsonSerializer::serialize_to_string(self),
            blobs: vec![],
        }
    }

    fn decode(
        payload: &StreamingAssetPayload,
        _package: &mut StreamingAssetFile,
    ) -> streaming::error::Result<Self> {
        let root = payload.data.expect_object("prefab metadata root")?;

        let nodes_value = root.required_field("nodes")?;
        let nodes = nodes_value.expect_parse("prefab nodes")?;

        Ok(PrefabAsset {
            source: root
                .required_field("source")?
                .expect_parse("prefab source")?,
            root_nodes: root
                .required_field("root_nodes")?
                .expect_parse("prefab root_nodes")?,
            nodes,
            animation_assets: root
                .required_field("animation_assets")?
                .expect_parse("prefab animation_assets")?,
        })
    }
}

impl StreamableAsset for PrefabMaterial {
    fn encode(&self) -> BuiltPayload {
        BuiltPayload {
            payload: JsonSerializer::serialize_to_string(self),
            blobs: vec![],
        }
    }

    fn decode(
        payload: &StreamingAssetPayload,
        _package: &mut StreamingAssetFile,
    ) -> streaming::error::Result<Self> {
        let root = payload
            .data
            .expect_object("prefab material metadata root")?;

        Ok(PrefabMaterial {
            name: root.required_field("name")?.expect_parse("material name")?,
            base_color: root
                .required_field("base_color")?
                .expect_parse("material base_color")?,
            metallic: root
                .required_field("metallic")?
                .expect_parse("material metallic")?,
            roughness: root
                .required_field("roughness")?
                .expect_parse("material roughness")?,
            alpha_cutoff: root
                .optional_field("alpha_cutoff")
                .expect_parse("material alpha_cutoff")?,
            alpha_mode: root
                .required_field("alpha_mode")?
                .expect_parse("material alpha_mode")?,
            double_sided: root
                .required_field("double_sided")?
                .expect_parse("material double_sided")?,
            unlit: root
                .required_field("unlit")?
                .expect_parse("material unlit")?,
            emissive_factor: root
                .required_field("emissive_factor")?
                .expect_parse("material emissive_factor")?,
            base_color_texture: root
                .optional_field("base_color_texture")
                .expect_parse("material base_color_texture")?,
            normal_texture: root
                .optional_field("normal_texture")
                .expect_parse("material normal_texture")?,
            metallic_roughness_texture: root
                .optional_field("metallic_roughness_texture")
                .expect_parse("material metallic_roughness_texture")?,
            emissive_texture: root
                .optional_field("emissive_texture")
                .expect_parse("material emissive_texture")?,
            occlusion_texture: root
                .optional_field("occlusion_texture")
                .expect_parse("material occlusion_texture")?,
        })
    }
}
