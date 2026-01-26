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
pub use ::winit;

#[cfg(feature = "derive")]
pub use ::syrillian_macros;

#[cfg(feature = "derive")]
pub use ::syrillian_macros::{Reflect, SyrillianApp, reflect_fn};
