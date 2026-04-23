use syrillian::World;
use syrillian::core::GameObjectId;
use syrillian::core::component_factory::ComponentFactory;
use syrillian::core::reflection::{ReflectSerialize, Value};
use syrillian::reflect::serializer::JsonSerializer;
use syrillian_asset::{PrefabAsset, PrefabComponent, PrefabNode};

/// Serializes a world object hierarchy into a PrefabAsset.
///
/// Only reflected component fields are persisted. Components without reflection data
/// are skipped. Transforms and hierarchy structure are always saved.
pub struct SceneSaver;

impl SceneSaver {
    /// Save an object and all its children as a PrefabAsset.
    pub fn save(world: &World, root_id: GameObjectId) -> Option<PrefabAsset> {
        let mut nodes = Vec::new();
        let root_index = Self::collect_node(world, root_id, &mut nodes)?;

        Some(PrefabAsset {
            source: String::new(),
            root_nodes: vec![root_index],
            nodes,
            animation_assets: vec![],
        })
    }

    /// Save multiple root objects as a single PrefabAsset.
    pub fn save_roots(world: &World, root_ids: &[GameObjectId]) -> PrefabAsset {
        let mut nodes = Vec::new();
        let mut root_indices = Vec::new();

        for &root_id in root_ids {
            if let Some(index) = Self::collect_node(world, root_id, &mut nodes) {
                root_indices.push(index);
            }
        }

        PrefabAsset {
            source: String::new(),
            root_nodes: root_indices,
            nodes,
            animation_assets: vec![],
        }
    }

    fn collect_node(world: &World, id: GameObjectId, nodes: &mut Vec<PrefabNode>) -> Option<u32> {
        let object = world.objects.get(id)?;

        let pos = object.transform.local_position();
        let rot = object.transform.local_rotation();
        let scale = object.transform.local_scale();

        // Serialize reflected components
        let mut components = Vec::new();
        for comp_ref in object.iter_dyn_components() {
            let Some(comp_type_info) = comp_ref.type_info() else {
                continue;
            };

            // Only serialize components that have reflected fields
            if comp_type_info.fields.is_empty() {
                continue;
            }

            let Some(factory_entry) = ComponentFactory::find_by_type_id(comp_type_info.type_id)
            else {
                continue;
            };

            let serialized = ReflectSerialize::serialize(comp_ref);
            if let Value::Object(fields) = serialized {
                components.push(PrefabComponent {
                    type_name: factory_entry.full_path.to_string(),
                    fields,
                });
            }
        }

        // Reserve index for this node
        let node_index = nodes.len() as u32;
        nodes.push(PrefabNode::default()); // placeholder

        // Recursively collect children
        let mut child_indices = Vec::new();
        for &child_id in object.children() {
            if let Some(child_index) = Self::collect_node(world, child_id, nodes) {
                child_indices.push(child_index);
            }
        }

        // Serialize custom properties if any
        let extras_json = if !object.properties().is_empty() {
            let props_value = Value::Object(
                object
                    .properties()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            );
            Some(JsonSerializer::value_to_string(&props_value))
        } else {
            None
        };

        // Fill in the actual node data
        nodes[node_index as usize] = PrefabNode {
            name: object.name.clone(),
            local_position: *pos,
            local_rotation: *rot,
            local_scale: *scale,
            children: child_indices,
            mesh: None, // TODO: serialize mesh bindings
            extras_json,
            components,
        };

        Some(node_index)
    }
}
