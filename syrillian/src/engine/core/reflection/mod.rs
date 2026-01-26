pub mod impl_nalgebra;
pub mod impl_primitive;
pub mod macros;
mod serialize_builtins;
mod serialize_extern;
mod serialize_primitive;
mod serialize_std;
pub mod serializer;

pub use serialize_primitive::Value;

use dashmap::DashMap;
use std::any::TypeId;
use std::collections::BTreeMap;
use std::sync::{Once, OnceLock};
use tracing::warn;

#[derive(Copy, Clone, Debug)]
pub struct ReflectedField {
    pub name: &'static str,
    pub offset: usize,
    pub type_id: TypeId,
}

#[derive(Copy, Clone, Debug)]
pub struct ReflectedTypeActions {
    pub serialize: fn(*const u8) -> Value,
}

#[derive(Copy, Clone, Debug)]
pub struct ReflectedTypeInfo {
    pub type_id: TypeId,
    pub full_path: &'static str,
    pub name: &'static str,
    pub fields: &'static [ReflectedField],
    pub actions: ReflectedTypeActions,
}

pub trait ReflectSerialize {
    fn serialize(this: &Self) -> Value
    where
        Self: Sized;
}

impl ReflectedTypeInfo {
    pub fn new_of<T: ReflectSerialize + 'static>() -> Self {
        let type_name = std::any::type_name::<T>();
        let base_name = type_name.split('<').next().unwrap_or(type_name);
        let short_name = base_name.rsplit("::").next().unwrap_or(base_name);

        Self {
            type_id: TypeId::of::<T>(),
            full_path: type_name,
            name: short_name,
            actions: ReflectedTypeActions {
                serialize: serialize_as::<T>,
            },
            fields: &[],
        }
    }
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

pub unsafe fn field<R, T>(this: &R, name: &str) -> Option<*const T>
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

impl<T: PartialReflect + 'static> Reflect for T {}

pub trait Reflect
where
    Self: 'static,
{
    fn field_ref<'a, T: 'static>(this: &'a Self, name: &str) -> Option<&'a T>
    where
        Self: Sized + 'static,
    {
        let field_ptr = unsafe { field(this, name)? };
        Some(unsafe { &*field_ptr })
    }

    fn field_mut<'a, T: 'static>(this: &'a mut Self, name: &str) -> Option<&'a mut T>
    where
        Self: Sized + 'static,
    {
        let field_ptr = unsafe { field::<_, T>(this, name)? as *mut T };
        Some(unsafe { &mut *field_ptr })
    }

    fn reflected_info() -> Option<ReflectedTypeInfo> {
        type_info_of::<Self>()
    }

    fn reflected_fields() -> Option<&'static [ReflectedField]>
    where
        Self: Sized + 'static,
    {
        Some(type_info_of::<Self>()?.fields)
    }
}

pub fn serialize_as<T: ReflectSerialize>(ptr: *const u8) -> Value {
    let value: &T = unsafe { &*(ptr as *const T) };
    ReflectSerialize::serialize(value)
}

impl<R: Reflect> ReflectSerialize for R {
    fn serialize(this: &Self) -> Value {
        let mut map = BTreeMap::new();
        let Some(type_data) = Self::reflected_info() else {
            return Value::None;
        };
        let base = this as *const _ as usize;
        for field in type_data.fields {
            let Some(ty) = type_info(field.type_id) else {
                warn!(
                    "Type of {}::{} was requested for serialization but didn't have reflection data",
                    type_data.name, field.name,
                );
                continue;
            };

            let field_ptr = (base + field.offset) as *const u8;

            map.insert(field.name.to_string(), (ty.actions.serialize)(field_ptr));
        }
        Value::Map(map)
    }
}
