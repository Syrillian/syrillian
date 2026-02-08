//! GPU rendering backend built on top of `wgpu`.
//!
//! The rendering module is responsible for preparing GPU resources,
//! building command buffers and drawing the world each frame.
//!
//! As a user, you don't usually need to interact with the things in here,
//! besides the [`AssetCache`].
//!
//! See documentation of [`World`](syrillian::engine::world::World) to find out how to add GPU data.
//!
//! To retrieve meshes, or other things, you'll use Handles, defined as [`H<T>`](syrillian_asset::H),
//! but for cleanliness it's appropriate to use the types like [`HMesh`](syrillian_asset::HMesh).
//!
//! These handles are indices into the [`AssetStore`](syrillian_asset::AssetStore), and serve as a
//! combined handle into the [`AssetCache`]. The [`AssetStore`](syrillian_asset::AssetStore) is
//! where you can put your raw data, which is then initialized by the AssetCache on the GPU.
//!
//! You'll usually only interact with the Cache or something like that, in a
//! [`Render Proxy`](proxies::SceneProxy), which is Syrillians abstraction for
//! render-side drawable world state, which knows how to render the state the proxy represents.
//!
//! In a [`Scene Proxy`](proxies::SceneProxy) you'll want to interact with the
//! [`GPUDrawCtx`] object that contains all info for the frame, and inner-frame draw call.
//!
//! Creating a scene proxy is or >should< usually not be necessary as you can just spawn a builtin
//! scene proxy in your Component
//!
//! You can create scene proxies in [`Components`](syrillian::engine::components)

pub mod context;
pub mod message;
pub mod offscreen_surface;
pub mod picking;
pub mod render_data;
pub mod renderer;
pub mod state;
pub mod texture_export;
pub mod uniform;

pub mod debug_renderer;
pub mod viewport;

pub use crate::cache::*;
pub use context::*;
pub use wgpu::TextureFormat;
