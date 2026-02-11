extern crate self as syrillian;

pub mod engine;
pub mod math;
pub mod utils;
pub mod windowing;

pub use engine::*;
pub use rendering::strobe;
pub use windowing::*;

pub use ::gilrs;
pub use ::inventory;
pub use ::tracing;
pub use ::wgpu;
pub use ::winit;

pub use ::syrillian_asset as assets;
pub use ::syrillian_macros;
pub use ::syrillian_render as rendering;
pub use ::syrillian_shadergen as shadergen;

#[cfg(feature = "derive")]
pub use ::syrillian_macros::SyrillianApp;

pub use ::syrillian_macros::{Reflect, reflect_fn};
