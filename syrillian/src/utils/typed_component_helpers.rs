use crate::components::Component;
use crate::core::reflection::{ReflectedTypeInfo, type_info};
use slotmap::Key;
use std::any::TypeId;
use syrillian_utils::{ComponentId, TypedComponentId};

pub trait TypedComponentHelper {
    fn is_a<C: Component>(&self) -> bool;
    fn type_info(&self) -> Option<ReflectedTypeInfo>;
    fn type_name(&self) -> Option<&'static str>;
    fn short_name(&self) -> Option<&'static str>;
    fn null<C: Component + ?Sized>() -> TypedComponentId;
    fn from_typed<C: Component + ?Sized>(id: ComponentId) -> Self;
}

impl TypedComponentHelper for TypedComponentId {
    fn is_a<C: Component>(&self) -> bool {
        self.type_id() == TypeId::of::<C>()
    }
    fn type_info(&self) -> Option<ReflectedTypeInfo> {
        type_info(self.type_id())
    }

    fn type_name(&self) -> Option<&'static str> {
        self.type_info().map(|info| info.full_path)
    }

    fn short_name(&self) -> Option<&'static str> {
        self.type_info().map(|info| info.name)
    }

    fn null<C: Component + ?Sized>() -> TypedComponentId {
        Self::from_typed::<C>(ComponentId::null())
    }

    fn from_typed<C: Component + ?Sized>(id: ComponentId) -> Self {
        TypedComponentId::new_raw(TypeId::of::<C>(), id)
    }
}
