use crate::rendering::state::StateError;
use snafu::Snafu;
use wgpu::SurfaceError;

pub type Result<T, E = RenderError> = std::result::Result<T, E>;

#[derive(Debug, Snafu)]
#[snafu(context(suffix(Err)), visibility(pub(crate)))]
pub enum RenderError {
    #[snafu(display("Render data should've been set"))]
    DataNotSet,

    #[snafu(display("Render pipeline is not set"))]
    NoRenderPipeline,

    #[snafu(display("Invalid Shader requested"))]
    InvalidShader,

    #[snafu(display("No camera set for rendering"))]
    NoCameraSet,

    #[snafu(display("Rendering camera doesn't have a camera component"))]
    NoCameraComponentSet,

    #[snafu(display("Light UBGL was not created"))]
    NoLightUBGL,

    #[snafu(display("Error with current render surface: {source}"))]
    Surface { source: SurfaceError },

    #[snafu(display("Failed to create render state: {source}"))]
    State { source: StateError },
}
