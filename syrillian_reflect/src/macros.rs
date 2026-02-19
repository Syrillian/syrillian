#[macro_export]
macro_rules! reflect_type_info {
    (primitive, $type_name:ty) => {
        ::syrillian_reflect::ReflectedTypeInfo {
            type_id: std::any::TypeId::of::<$type_name>(),
            full_path: stringify!($type_name),
            name: stringify!($type_name),
            actions: ::syrillian_reflect::ReflectedTypeActions {
                serialize: ::syrillian_reflect::serialize_as::<$type_name>,
            },
            fields: &[],
        }
    };

    ($path:path, $type_name:ty, $fields:expr) => {
        ::syrillian_reflect::ReflectedTypeInfo {
            type_id: std::any::TypeId::of::<$type_name>(),
            full_path: concat!(stringify!($path), "::", stringify!($type_name)),
            name: stringify!($type_name),
            actions: ::syrillian_reflect::ReflectedTypeActions {
                serialize: ::syrillian_reflect::serialize_as::<$type_name>,
            },
            fields: $fields,
        }
    };
}

#[macro_export]
macro_rules! impl_reflect {
    ($path:path, $type_name:ty, $fields:expr) => {
        impl ::syrillian_reflect::PartialReflect for $type_name {
            const DATA: ::syrillian_reflect::ReflectedTypeInfo =
                ::syrillian_reflect::reflect_type_info!($path, $type_name, $fields);
        }
    };
}

#[macro_export]
macro_rules! impl_reflect_generic {
    ($path:path, $type_name:ident<[$( $generics:ty ),*]>, $fields:expr) => {
        $(
            ::syrillian_reflect::impl_reflect!($path, $type_name<$generics>, $fields);
        )*
    };
}

#[macro_export]
macro_rules! reflect_field {
    ($offset_type:ty, $name:ident, $field_type:ty) => {
        ::syrillian_reflect::ReflectedField {
            name: stringify!($name),
            offset: std::mem::offset_of!($offset_type, $name),
            type_id: std::any::TypeId::of::<$field_type>(),
        }
    };
}

#[macro_export]
macro_rules! register_type {
    ($( $type_info:tt )*) => {
        ::syrillian_reflect::inventory::submit! {
            $( $type_info )*
        }
    };
}
