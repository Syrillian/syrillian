use slotmap::new_key_type;
use std::any::TypeId;

pub use slotmap::Key;

new_key_type! { pub struct ComponentId; }

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct TypedComponentId(TypeId, ComponentId);

impl From<TypedComponentId> for ComponentId {
    fn from(value: TypedComponentId) -> Self {
        value.1
    }
}

impl TypedComponentId {
    // This function is usually only used in the internals and similarly only useful within
    // a component storage implementation.
    pub fn new<T: 'static>(component_id: ComponentId) -> Self {
        Self(TypeId::of::<T>(), component_id)
    }

    // This function is usually only used in the internals and similarly only useful within
    // a component storage implementation.
    pub fn new_raw(type_id: TypeId, component_id: ComponentId) -> Self {
        Self(type_id, component_id)
    }

    pub fn type_id(&self) -> TypeId {
        self.0
    }

    pub fn component_id(&self) -> ComponentId {
        self.1
    }
}
