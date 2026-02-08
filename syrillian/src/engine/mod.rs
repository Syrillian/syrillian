//! Root module of the Syrillian game engine.
//!
//! It exposes all primary systems such as rendering, physics and the
//! asset pipeline. Applications usually interact with the [`World`]
//! type defined here to spawn and manage game objects.

pub mod components;
pub mod core;
pub mod input;
pub mod physics;
pub mod reflection;
pub mod world;

pub mod audio;
pub mod prefabs;

pub use self::world::World;

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const ENGINE_NAME: &str = env!("CARGO_PKG_NAME");
pub const ENGINE_BUILD_DATE: &str = env!("BUILD_DATE");
pub const ENGINE_BUILD_TIME: &str = env!("BUILD_TIME");
pub const ENGINE_BUILD_HASH: &str = env!("GIT_HASH");

pub const ENGINE_STR: &str = const_format::concatcp!(
    ENGINE_NAME,
    " - v.",
    ENGINE_VERSION,
    " - built on ",
    ENGINE_BUILD_DATE,
    " at ",
    ENGINE_BUILD_TIME
);
