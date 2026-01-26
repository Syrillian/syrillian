//! Handling of keyboard and mouse input.
//!
//! [`InputManager`] tracks key states and mouse movement and is used by
//! components and systems to react to user interaction.

mod gamepad_manager;
pub mod input_manager;

pub use self::input_manager::*;

pub use gilrs::{Axis, Button};
pub use winit::event::MouseButton;
pub use winit::keyboard::KeyCode;
