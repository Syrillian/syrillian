//! Core data structures used throughout the engine.
//!
//! This includes game objects, their transforms and vertex types used for
//! rendering.

pub mod bone;
pub mod bounds;
pub(super) mod component_context_inference;
pub mod component_storage;
pub mod object;
pub mod object_extensions;
pub mod transform;
pub mod vertex;

pub use bone::*;
pub use bounds::*;
pub use object::*;
pub use object_extensions::*;
pub use transform::*;
pub use vertex::*;
