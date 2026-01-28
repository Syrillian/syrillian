//! Physics simulation powered by `rapier`.
//!
//! The [`PhysicsSimulation`] struct manages rigid bodies / joints, etc.
//! and executes physics steps each frame.

pub mod simulator;

pub use simulator::*;

pub use ::rapier3d;

pub use ::rapier3d::geometry::Ray;
pub use ::rapier3d::pipeline::QueryFilter;
