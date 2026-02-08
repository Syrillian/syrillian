//! Core data structures used throughout the engine.
//!
//! This includes game objects, their transforms and vertex types used for
//! rendering.

pub(super) mod component_context_inference;
pub mod component_storage;
pub mod object;
pub mod object_extensions;
pub mod reflection;
pub mod transform;

pub use object::*;
pub use object_extensions::*;
pub use syrillian_asset::mesh::bone::*;
pub use syrillian_asset::mesh::vertex::*;
pub use syrillian_utils::BoundingSphere;
pub use syrillian_utils::Frustum;
pub use transform::*;
