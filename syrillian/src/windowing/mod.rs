//! Platform windowing and event loop utilities.
//!
//! These helpers abstract the details of the `winit` window creation and
//! application state management into a compact runtime that can be easily used.

pub mod app;
pub mod game_thread;
pub mod presenter;
pub mod render_thread;
pub mod state;

pub use app::*;
pub use presenter::*;
pub use render_thread::*;
pub use state::*;
pub use winit::dpi::PhysicalSize;
