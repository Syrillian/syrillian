use dashmap::DashMap;
use std::any::TypeId;
use std::sync::{Once, OnceLock};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ReflectedField {
    pub name: &'static str,
    pub offset: usize,
    pub type_id: TypeId,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ReflectedTypeInfo {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub short_name: &'static str,
    pub fields: &'static [ReflectedField],
}

inventory::collect!(ReflectedTypeInfo);

static TYPE_REGISTRY: OnceLock<DashMap<TypeId, ReflectedTypeInfo>> = OnceLock::new();
static TYPE_INVENTORY_LOADED: Once = Once::new();

fn type_registry() -> &'static DashMap<TypeId, ReflectedTypeInfo> {
    TYPE_REGISTRY.get_or_init(DashMap::new)
}

fn load_type_inventory() {
    TYPE_INVENTORY_LOADED.call_once(|| {
        for info in inventory::iter::<ReflectedTypeInfo> {
            type_registry().entry(info.type_id).or_insert(*info);
        }
    });
}

pub fn preload_type_reflections() {
    load_type_inventory();
}

pub fn type_info(type_id: TypeId) -> Option<ReflectedTypeInfo> {
    load_type_inventory();
    type_registry().get(&type_id).map(|entry| *entry)
}

#[inline]
pub fn type_info_of<T: ?Sized + 'static>() -> Option<ReflectedTypeInfo> {
    type_info(TypeId::of::<T>())
}

pub fn type_infos() -> Vec<ReflectedTypeInfo> {
    load_type_inventory();
    type_registry().iter().map(|entry| *entry).collect()
}

fn field<R, T>(this: &R, name: &str) -> Option<*const T>
where
    R: Sized + Reflect + 'static,
    T: 'static,
{
    let type_info = type_info_of::<R>()?;
    let field = type_info.fields.iter().find(|f| f.name == name)?;
    if field.type_id != TypeId::of::<T>() {
        return None;
    }
    let base = this as *const R;
    Some(unsafe { base.byte_add(field.offset) as *const T })
}

pub trait PartialReflect {
    const DATA: ReflectedTypeInfo;
}

impl<T: PartialReflect> Reflect for T {}

pub trait Reflect {
    fn field_ref<'a, T: 'static>(this: &'a Self, name: &str) -> Option<&'a T>
    where
        Self: Sized + 'static,
    {
        let field_ptr = field(this, name)?;
        Some(unsafe { &*field_ptr })
    }

    fn field_mut<'a, T: 'static>(this: &'a mut Self, name: &str) -> Option<&'a mut T>
    where
        Self: Sized + 'static,
    {
        let field_ptr = field::<_, T>(this, name)? as *mut T;
        Some(unsafe { &mut *field_ptr })
    }
}
