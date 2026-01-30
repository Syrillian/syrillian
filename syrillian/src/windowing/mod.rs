//! Platform windowing and event loop utilities.
//!
//! These helpers abstract the details of the `winit` window creation and
//! application state management into a compact runtime that can be easily used.

pub mod app;
pub mod game_thread;
pub mod presenter;
pub mod render_thread;
pub mod state;

use crate::assets::HRenderTexture2D;
pub use app::*;
pub use presenter::*;
pub use render_thread::*;
pub use state::*;
pub use winit::dpi::PhysicalSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ViewportId(pub u64);

impl ViewportId {
    pub const PRIMARY: Self = Self(0);

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn is_primary(self) -> bool {
        self.get() == Self::PRIMARY.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RenderTarget {
    Viewport(ViewportId),
    Texture(HRenderTexture2D),
}

impl RenderTarget {
    pub const PRIMARY_WINDOW: Self = Self::Viewport(ViewportId::PRIMARY);
}
