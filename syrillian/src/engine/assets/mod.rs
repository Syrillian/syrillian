//! Asset loading and management utilities.
//!
//! Assets such as meshes, textures and shaders are stored in type specific
//! stores and referenced through handles. This module also exposes helper
//! functionality for loading scenes.
//!
//! Example on how to interact with the store:
//! ```rust
//! use syrillian::assets::{HMaterial, Material};
//! use syrillian::World;
//!
//! fn update(world: &mut World) {
//!     // make a Material
//!     let material: Material = Material::builder()
//!         .name("Test Material")
//!         .build();
//!
//!     // add an asset
//!     let material: HMaterial = world.assets.materials.add(material);
//! }
//! ```
//!
//! To see how you can use an asset on the GPU, check [`AssetCache`](crate::rendering::AssetCache)

mod asset_store;
pub(crate) mod generic_store;

mod bind_group_layout;
mod font;
mod material;
mod mesh;
mod shader;
mod texture_2d;

mod generic_texture;
mod handle;
mod key;
mod render_texture_2d;
mod render_texture_2d_array;
mod sound;
mod texture_2d_array;

pub use self::asset_store::*;
pub use self::handle::*;

pub use self::bind_group_layout::*;
pub use self::font::*;
pub use self::material::*;
pub use self::mesh::*;
pub use self::render_texture_2d::*;
pub use self::render_texture_2d_array::*;
pub use self::shader::*;
pub use self::sound::*;
pub use self::texture_2d::*;
pub use self::texture_2d_array::*;

pub use generic_store::StoreType;

pub(crate) use self::generic_store::*;
pub(crate) use self::generic_texture::*;
pub(crate) use self::key::*;

pub type HBGL = H<BGL>;
pub type HMaterial = H<Material>;
pub type HMesh = H<Mesh>;
pub type HShader = H<Shader>;
pub type HTexture2D = H<Texture2D>;
pub type HTexture2DArray = H<Texture2DArray>;
pub type HRenderTexture2D = H<RenderTexture2D>;
pub type HRenderTexture2DArray = H<RenderTexture2DArray>;
pub type HFont = H<Font>;
pub type HSound = H<Sound>;
