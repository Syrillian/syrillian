use crate::assets::mesh::{Bones, Mesh};
use crate::assets::prefab::{PrefabAsset, PrefabMaterial, PrefabMeshBinding, PrefabNode};
use crate::assets::shader::{Shader, ShaderCode, ShaderType};
use crate::assets::texture_2d::Texture2D;
use crate::mesh::{SkinnedVertex3D, UnskinnedVertex3D};
use crate::{AnimationChannel, AnimationClip, SkinnedMesh, TransformKeys};
use std::collections::BTreeMap;
use syrillian_reflect::{ReflectSerialize, Value};

impl ReflectSerialize for SkinnedVertex3D {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "position".to_string(),
                ReflectSerialize::serialize(&this.position),
            ),
            ("uv".to_string(), ReflectSerialize::serialize(&this.uv)),
            (
                "normal".to_string(),
                ReflectSerialize::serialize(&this.normal),
            ),
            (
                "tangent".to_string(),
                ReflectSerialize::serialize(&this.tangent.into_inner()),
            ),
            (
                "bone_indices".to_string(),
                ReflectSerialize::serialize(&this.bone_indices.to_vec()),
            ),
            (
                "bone_weights".to_string(),
                ReflectSerialize::serialize(&this.bone_weights.to_vec()),
            ),
        ]))
    }
}

impl ReflectSerialize for UnskinnedVertex3D {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "position".to_string(),
                ReflectSerialize::serialize(&this.position),
            ),
            ("uv".to_string(), ReflectSerialize::serialize(&this.uv)),
            (
                "normal".to_string(),
                ReflectSerialize::serialize(&this.normal),
            ),
            (
                "tangent".to_string(),
                ReflectSerialize::serialize(&this.tangent),
            ),
        ]))
    }
}

impl ReflectSerialize for Bones {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "names".to_string(),
                ReflectSerialize::serialize(&this.names),
            ),
            (
                "parents".to_string(),
                ReflectSerialize::serialize(&this.parents),
            ),
            (
                "children".to_string(),
                ReflectSerialize::serialize(&this.children),
            ),
            (
                "roots".to_string(),
                ReflectSerialize::serialize(&this.roots),
            ),
            (
                "index_of".to_string(),
                ReflectSerialize::serialize(&this.index_of),
            ),
        ]))
    }
}

impl ReflectSerialize for Mesh {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "material_ranges".to_string(),
                ReflectSerialize::serialize(&this.material_ranges),
            ),
            (
                "bounding_sphere".to_string(),
                match this.bounding_sphere {
                    None => Value::None,
                    Some(ref b) => Value::Object(BTreeMap::from([
                        ("center".to_string(), ReflectSerialize::serialize(&b.center)),
                        ("radius".to_string(), Value::Float(b.radius)),
                    ])),
                },
            ),
        ]))
    }
}

impl ReflectSerialize for SkinnedMesh {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "material_ranges".to_string(),
                ReflectSerialize::serialize(&this.material_ranges),
            ),
            (
                "bones".to_string(),
                ReflectSerialize::serialize(&this.bones),
            ),
            (
                "bounding_sphere".to_string(),
                match this.bounding_sphere {
                    None => Value::None,
                    Some(ref b) => Value::Object(BTreeMap::from([
                        ("center".to_string(), ReflectSerialize::serialize(&b.center)),
                        ("radius".to_string(), Value::Float(b.radius)),
                    ])),
                },
            ),
        ]))
    }
}

impl ReflectSerialize for Texture2D {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            ("width".to_string(), Value::UInt(this.width)),
            ("height".to_string(), Value::UInt(this.height)),
            (
                "format".to_string(),
                Value::String(format!("{:?}", this.format)),
            ),
            (
                "repeat_mode".to_string(),
                Value::String(format!("{:?}", this.repeat_mode)),
            ),
            (
                "filter_mode".to_string(),
                Value::String(format!("{:?}", this.filter_mode)),
            ),
            (
                "mip_filter_mode".to_string(),
                Value::String(format!("{:?}", this.mip_filter_mode)),
            ),
            (
                "has_transparency".to_string(),
                Value::Bool(this.has_transparency),
            ),
        ]))
    }
}

impl ReflectSerialize for ShaderCode {
    fn serialize(this: &Self) -> Value {
        match this {
            ShaderCode::Full(source) => Value::Object(BTreeMap::from([
                ("kind".to_string(), Value::String("Full".to_string())),
                ("source".to_string(), Value::String(source.clone())),
            ])),
            ShaderCode::Fragment(source) => Value::Object(BTreeMap::from([
                ("kind".to_string(), Value::String("Fragment".to_string())),
                ("source".to_string(), Value::String(source.clone())),
            ])),
        }
    }
}

impl ReflectSerialize for ShaderType {
    fn serialize(this: &Self) -> Value {
        let value = match this {
            ShaderType::Default => "Default",
            ShaderType::Custom => "Custom",
            ShaderType::PostProcessing => "PostProcessing",
        };
        Value::String(value.to_string())
    }
}

impl ReflectSerialize for Shader {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            ("name".to_string(), Value::String(this.name().to_string())),
            ("code".to_string(), ReflectSerialize::serialize(this.code())),
            (
                "shader_type".to_string(),
                ReflectSerialize::serialize(&this.stage()),
            ),
            (
                "polygon_mode".to_string(),
                Value::String(format!("{:?}", this.polygon_mode())),
            ),
            (
                "topology".to_string(),
                Value::String(format!("{:?}", this.topology())),
            ),
            (
                "immediate_size".to_string(),
                Value::UInt(this.immediate_size()),
            ),
            (
                "depth_enabled".to_string(),
                Value::Bool(this.is_depth_enabled()),
            ),
            (
                "shadow_transparency".to_string(),
                Value::Bool(this.has_shadow_transparency()),
            ),
        ]))
    }
}

impl ReflectSerialize for PrefabMeshBinding {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "mesh_asset".to_string(),
                Value::String(this.mesh_asset.clone()),
            ),
            (
                "material_hashes".to_string(),
                ReflectSerialize::serialize(&this.material_hashes),
            ),
        ]))
    }
}

impl ReflectSerialize for TransformKeys {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "t_times_count".to_string(),
                Value::BigUInt(this.t_times.len() as u64),
            ),
            (
                "t_values_count".to_string(),
                Value::BigUInt(this.t_values.len() as u64),
            ),
            (
                "r_times_count".to_string(),
                Value::BigUInt(this.r_times.len() as u64),
            ),
            (
                "r_values_count".to_string(),
                Value::BigUInt(this.r_values.len() as u64),
            ),
            (
                "s_times_count".to_string(),
                Value::BigUInt(this.s_times.len() as u64),
            ),
            (
                "s_values_count".to_string(),
                Value::BigUInt(this.s_values.len() as u64),
            ),
        ]))
    }
}

impl ReflectSerialize for AnimationChannel {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "target_name".to_string(),
                Value::String(this.target_name.clone()),
            ),
            ("keys".to_string(), ReflectSerialize::serialize(&this.keys)),
        ]))
    }
}

impl ReflectSerialize for AnimationClip {
    fn serialize(this: &Self) -> Value {
        let channels = this
            .channels
            .iter()
            .map(ReflectSerialize::serialize)
            .collect::<Vec<_>>();

        Value::Object(BTreeMap::from([
            ("name".to_string(), Value::String(this.name.clone())),
            ("duration".to_string(), Value::Float(this.duration)),
            ("channels".to_string(), Value::Array(channels)),
        ]))
    }
}

impl ReflectSerialize for PrefabNode {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            ("name".to_string(), Value::String(this.name.clone())),
            (
                "local_position".to_string(),
                ReflectSerialize::serialize(&this.local_position),
            ),
            (
                "local_rotation".to_string(),
                ReflectSerialize::serialize(&this.local_rotation),
            ),
            (
                "local_scale".to_string(),
                ReflectSerialize::serialize(&this.local_scale),
            ),
            (
                "children".to_string(),
                ReflectSerialize::serialize(&this.children),
            ),
            ("mesh".to_string(), ReflectSerialize::serialize(&this.mesh)),
            (
                "extras_json".to_string(),
                ReflectSerialize::serialize(&this.extras_json),
            ),
        ]))
    }
}

impl ReflectSerialize for PrefabMaterial {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            ("name".to_string(), Value::String(this.name.clone())),
            (
                "base_color".to_string(),
                ReflectSerialize::serialize(&this.base_color),
            ),
            ("metallic".to_string(), Value::Float(this.metallic)),
            ("roughness".to_string(), Value::Float(this.roughness)),
            (
                "alpha_cutoff".to_string(),
                ReflectSerialize::serialize(&this.alpha_cutoff),
            ),
            (
                "alpha_mode".to_string(),
                Value::String(this.alpha_mode.clone()),
            ),
            ("double_sided".to_string(), Value::Bool(this.double_sided)),
            ("unlit".to_string(), Value::Bool(this.unlit)),
            (
                "emissive_factor".to_string(),
                ReflectSerialize::serialize(&this.emissive_factor),
            ),
            (
                "base_color_texture".to_string(),
                ReflectSerialize::serialize(&this.base_color_texture),
            ),
            (
                "normal_texture".to_string(),
                ReflectSerialize::serialize(&this.normal_texture),
            ),
            (
                "metallic_roughness_texture".to_string(),
                ReflectSerialize::serialize(&this.metallic_roughness_texture),
            ),
            (
                "emissive_texture".to_string(),
                ReflectSerialize::serialize(&this.emissive_texture),
            ),
            (
                "occlusion_texture".to_string(),
                ReflectSerialize::serialize(&this.occlusion_texture),
            ),
        ]))
    }
}

impl ReflectSerialize for PrefabAsset {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            ("source".to_string(), Value::String(this.source.clone())),
            (
                "root_nodes".to_string(),
                ReflectSerialize::serialize(&this.root_nodes),
            ),
            (
                "nodes".to_string(),
                ReflectSerialize::serialize(&this.nodes),
            ),
            (
                "animation_assets".to_string(),
                ReflectSerialize::serialize(&this.animation_assets),
            ),
        ]))
    }
}

syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets::mesh,
    SkinnedVertex3D,
    &[]
));
syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets::mesh,
    UnskinnedVertex3D,
    &[]
));
syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets::mesh,
    Bones,
    &[]
));
syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets,
    Mesh,
    &[]
));
syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets,
    Texture2D,
    &[]
));
syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets::shader,
    ShaderCode,
    &[]
));
syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets::shader,
    ShaderType,
    &[]
));
syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets,
    Shader,
    &[]
));
syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets,
    TransformKeys,
    &[]
));
syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets,
    AnimationChannel,
    &[]
));
syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets,
    AnimationClip,
    &[]
));

syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets,
    PrefabMeshBinding,
    &[]
));
syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets,
    PrefabNode,
    &[]
));
syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets,
    PrefabMaterial,
    &[]
));
syrillian_reflect::register_type!(syrillian_reflect::reflect_type_info!(
    syrillian_asset::assets,
    PrefabAsset,
    &[]
));
