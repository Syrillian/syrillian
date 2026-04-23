use crate::core::GameObjectId;
use std::any::TypeId;
use syrillian_reflect::Value;

/// A registered component type that can be spawned by name in the editor.
///
/// Components opt in to factory registration via `#[reflect(component)]` on their
/// `#[derive(Reflect)]`. Only types that are `Component + Default + Reflect` qualify.
pub struct ComponentFactoryEntry {
    /// Short type name (e.g. "Gravity")
    pub type_name: &'static str,
    /// Full module path (e.g. "mygame::components::Gravity")
    pub full_path: &'static str,
    /// Runtime TypeId of the component
    pub type_id: TypeId,
    /// Spawns a Default::default() component on the given object.
    /// Returns a raw pointer to the component data for field application.
    pub spawn_fn: fn(&mut GameObjectId) -> *mut u8,
    /// Creates a Default::default() component, applies reflected fields, then spawns it.
    /// This lets components observe deserialized field values during init.
    pub spawn_with_fields_fn: fn(&mut GameObjectId, &Value) -> *mut u8,
    /// Applies reflected field values from a Value onto the component.
    pub apply_fn: fn(*mut u8, &Value),
}

// Safety: ComponentFactoryEntry only contains static data and function pointers.
unsafe impl Send for ComponentFactoryEntry {}
unsafe impl Sync for ComponentFactoryEntry {}

inventory::collect!(ComponentFactoryEntry);

/// Registry of component types that can be instantiated by name.
pub struct ComponentFactory;

impl ComponentFactory {
    /// Returns all registered component factory entries.
    pub fn entries() -> Vec<&'static ComponentFactoryEntry> {
        inventory::iter::<ComponentFactoryEntry>
            .into_iter()
            .collect()
    }

    /// Find a factory entry by short type name.
    pub fn find(type_name: &str) -> Option<&'static ComponentFactoryEntry> {
        inventory::iter::<ComponentFactoryEntry>
            .into_iter()
            .find(|e| e.type_name == type_name)
    }

    /// Find a factory entry by short type name or full type path.
    pub fn find_by_name_or_path(type_name: &str) -> Option<&'static ComponentFactoryEntry> {
        Self::find(type_name).or_else(|| Self::find_by_path(type_name))
    }

    /// Find a factory entry by full type path.
    pub fn find_by_path(full_path: &str) -> Option<&'static ComponentFactoryEntry> {
        inventory::iter::<ComponentFactoryEntry>
            .into_iter()
            .find(|e| e.full_path == full_path)
    }

    /// Find a factory entry by TypeId.
    pub fn find_by_type_id(type_id: TypeId) -> Option<&'static ComponentFactoryEntry> {
        inventory::iter::<ComponentFactoryEntry>
            .into_iter()
            .find(|e| e.type_id == type_id)
    }

    /// Spawn a component by type name on the given object using Default::default().
    /// Returns true if the component was found and spawned.
    pub fn spawn(object: &mut GameObjectId, type_name: &str) -> bool {
        if let Some(entry) = Self::find_by_name_or_path(type_name) {
            (entry.spawn_fn)(object);
            true
        } else {
            false
        }
    }

    /// Spawn a component by type name and apply reflected field values.
    /// Returns true if the component was found and spawned.
    pub fn spawn_and_apply(object: &mut GameObjectId, type_name: &str, fields: &Value) -> bool {
        if let Some(entry) = Self::find_by_name_or_path(type_name) {
            (entry.spawn_with_fields_fn)(object, fields);
            true
        } else {
            false
        }
    }
}
