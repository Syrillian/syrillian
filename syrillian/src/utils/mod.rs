pub mod animation;
pub mod fat_ptr;
pub mod frame_counter;
pub(crate) mod hacks;
pub mod iter;
mod typed_component_helpers;
pub mod uniform_traits;

pub use frame_counter::*;
pub use syrillian_asset::assets::shader::checks::*;
pub use syrillian_asset::mesh::buffer;
pub use syrillian_utils::color::*;
pub use syrillian_utils::math::*;
pub use typed_component_helpers::TypedComponentHelper;

pub use syrillian_utils::{
    EngineArgs, ShaderUniformIndex, ShaderUniformMultiIndex, ShaderUniformSingleIndex,
};
