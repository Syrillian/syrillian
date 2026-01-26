//! Prefabricated objects that can be easily spawned into a [`World`](crate::World).
//!
//! Prefabs create game objects with common configurations such as a basic
//! camera or a textured cube.

mod camera;
mod prefab;

pub use camera::CameraPrefab;
pub use prefab::Prefab;
