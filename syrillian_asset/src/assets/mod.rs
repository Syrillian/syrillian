//! Asset loading and management utilities.
//!
//! Assets such as meshes, textures and shaders are stored in type specific
//! stores and referenced through handles. This module also exposes helper
//! functionality for loading scenes.
//!
//! Example on how to interact with the store:
//! ```ignore
//! use crate::{HMaterialInstance, MaterialInstance};
//! use syrillian::World;
//!
//! fn update(world: &mut World) {
//!     // make a Material Instance
//!     let material: MaterialInstance = MaterialInstance::builder()
//!         .name("Test Material Instance")
//!         .build();
//!
//!     // add an asset
//!     let material: HMaterialInstance = world.assets.material_instances.add(material);
//! }
//! ```
//!
//! To see how you can use an asset on the GPU, check [`AssetCache`](syrillian::engine::rendering::cache::asset_cache::AssetCache)

pub mod material_inputs;
pub mod mesh;
pub mod shader;

pub mod bind_group_layout;
pub mod cubemap;
pub mod font;
pub mod material;
pub mod material_instance;
pub mod sound;

pub mod render_cubemap;
pub mod render_texture_2d;
pub mod render_texture_2d_array;
pub mod texture_2d;
pub mod texture_2d_array;

pub use self::bind_group_layout::*;
pub use self::cubemap::*;
pub use self::font::Font;
pub use self::material::*;
pub use self::material_instance::*;
pub use self::mesh::Mesh;
pub use self::render_cubemap::*;
pub use self::render_texture_2d::*;
pub use self::render_texture_2d_array::*;
pub use self::shader::*;
pub use self::sound::*;
pub use self::texture_2d::*;
pub use self::texture_2d_array::*;

use crate::store::H;

pub type HBGL = H<BGL>;
pub type HMaterial = H<Material>;
pub type HMaterialInstance = H<MaterialInstance>;
pub type HMesh = H<Mesh>;
pub type HShader = H<Shader>;
pub type HTexture2D = H<Texture2D>;
pub type HTexture2DArray = H<Texture2DArray>;
pub type HCubemap = H<Cubemap>;
pub type HRenderTexture2D = H<RenderTexture2D>;
pub type HRenderTexture2DArray = H<RenderTexture2DArray>;
pub type HRenderCubemap = H<RenderCubemap>;
pub type HFont = H<Font>;
pub type HSound = H<Sound>;
