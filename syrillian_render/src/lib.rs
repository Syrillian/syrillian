extern crate self as syrillian_render;

pub mod cache;
pub mod error;
pub mod lighting;
pub mod model_uniform;
pub mod passes;
pub mod proxies;
pub mod rendering;
pub mod strobe;
mod utils;

type ObjectHash = u32;

pub use rendering::AssetCache;
pub use rendering::debug_renderer::DebugRenderer;
