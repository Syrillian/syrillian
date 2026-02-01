//! Built-in components that can be attached to a [`GameObject`](syrillian::core::GameObject).
//!
//! Components implement behavior ranging from camera control to physics. If it's dynamic,
//! it's probably a component.
//!
//! To make a component:
//! ```rust
//! use syrillian::math::Vec3;
//! use syrillian::components::Component;
//! use syrillian::core::GameObjectId;
//! use syrillian::World;
//!
//! pub struct Gravity {
//!     force: f32,
//! }
//!
//! impl Default for Gravity {
//!     fn default() -> Self {
//!         Gravity {
//!             force: 9.81,
//!         }
//!     }
//! }
//!
//! impl Component for Gravity {
//!     fn update(&mut self, world: &mut World) {
//!         let delta_time = world.delta_time().as_secs_f32();
//!
//!         let movement = Vec3::new(0.0, self.force * delta_time, 0.0);
//!
//!         let transform = &mut self.parent().transform;
//!         transform.translate(movement);
//!     }
//! }
//! ```

pub mod animation;
pub mod audio;
pub mod button;
pub mod collider;
pub mod flashlight;
pub mod fp_camera;
pub mod fp_movement;
pub mod freecam;
pub mod gravity;
pub mod joints;
pub mod light;
pub mod mesh_renderer;
pub mod rigid_body;
pub mod rotate;
pub mod skeletal;
pub mod text;

pub mod extensions;
pub mod prefabs;
pub mod profiler;

pub use animation::AnimationComponent;
pub use audio::{AudioEmitter, AudioReceiver};
pub use button::Button;
pub use collider::Collider3D;
pub use flashlight::FlashlightComponent;
pub use fp_camera::FirstPersonCameraController;
pub use fp_movement::FirstPersonMovementController;
pub use freecam::FreecamController;
pub use gravity::GravityComponent;
pub use joints::{
    FixedJoint, PrismaticJoint, RevoluteJoint, RopeJoint, SphericalJoint, SpringJoint,
};
pub use light::{PointLightComponent, SpotLightComponent, Sun, SunLightComponent};
pub use mesh_renderer::MeshRenderer;
pub use profiler::Profiler;
pub use rigid_body::RigidBodyComponent;
pub use rotate::RotateComponent;
pub use skeletal::SkeletalComponent;
pub use text::Text3D;
