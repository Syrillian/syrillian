use crate::core::reflection::Value;
use nalgebra::{ArrayStorage, Const, Matrix};

macro_rules! reflect_matrix {
    ($inner_type:ty, $num_r:literal, $num_c:literal) => {
        impl ::syrillian::core::reflection::ReflectSerialize for Matrix<$inner_type, Const<$num_r>, Const<$num_c>, ArrayStorage<$inner_type, $num_r, $num_c>> {
            fn serialize(this: &Self) -> Value {
                <ArrayStorage<$inner_type, $num_r, $num_c>>::serialize(&this.data)
            }
        }

        impl ::syrillian::core::reflection::ReflectSerialize for ArrayStorage<$inner_type, $num_r, $num_c> {
            fn serialize(this: &Self) -> Value {
                let mut list: Vec<Value> = Vec::new();
                for x in this.0 {
                    let mut inner_list = Vec::new();
                    for y in x {
                        inner_list.push(<$inner_type>::serialize(&y));
                    }
                    list.push(Value::Array(inner_list))
                }
                Value::Array(list)
            }
        }

        ::syrillian::register_type!(syrillian::reflect_type_info!(
            nalgebra,
            ArrayStorage<$inner_type, $num_r, $num_c>,
            &[]
        ));
        ::syrillian::register_type!(::syrillian::reflect_type_info!(
                nalgebra,
                Matrix<$inner_type, Const<$num_r>, Const<$num_c>, ArrayStorage<$inner_type, $num_r, $num_c>>,
                &[]
            ));
    };
    ($( $inner_types:ty ),* ; $num_r:literal, $num_c:literal) => {
        $(
            reflect_matrix!($inner_types, $num_r, $num_c);
        )*
    };
    ($num_r:literal, $num_c:literal) => {
        reflect_matrix!(i8, u8, i16, u16, i32, u32, i64, u64, usize, i128, u128, f32, f64 ; $num_r, $num_c);
    };
}

reflect_matrix!(1, 1);
reflect_matrix!(2, 1);
reflect_matrix!(3, 1);
reflect_matrix!(4, 1);

reflect_matrix!(1, 2);
reflect_matrix!(2, 2);
reflect_matrix!(3, 2);
reflect_matrix!(4, 2);

reflect_matrix!(1, 3);
reflect_matrix!(2, 3);
reflect_matrix!(3, 3);
reflect_matrix!(4, 3);

reflect_matrix!(1, 4);
reflect_matrix!(2, 4);
reflect_matrix!(3, 4);
reflect_matrix!(4, 4);
