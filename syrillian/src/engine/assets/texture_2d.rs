use crate::assets::generic_texture::TextureAsset;
use crate::engine::assets::generic_store::{HandleName, Store, StoreDefaults, StoreType};
use crate::engine::assets::{H, HTexture2D, StoreTypeFallback};
use crate::rendering::RenderMsg;
use crate::{World, store_add_checked};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use wgpu::{
    AddressMode, FilterMode, MipmapFilterMode, TextureDimension, TextureFormat, TextureUsages,
    TextureViewDimension,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture2D {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub data: Option<Vec<u8>>,
    pub repeat_mode: AddressMode,
    pub filter_mode: FilterMode,
    pub mip_filter_mode: MipmapFilterMode,
    pub has_transparency: bool,
}

impl H<Texture2D> {
    const FALLBACK_DIFFUSE_ID: u32 = 0;
    const FALLBACK_NORMAL_ID: u32 = 1;
    const FALLBACK_SHININESS_ID: u32 = 2;
    const MAX_BUILTIN_ID: u32 = 2;

    pub const FALLBACK_DIFFUSE: H<Texture2D> = H::new(Self::FALLBACK_DIFFUSE_ID);
    pub const FALLBACK_NORMAL: H<Texture2D> = H::new(Self::FALLBACK_NORMAL_ID);
    pub const FALLBACK_ROUGHNESS: H<Texture2D> = H::new(Self::FALLBACK_SHININESS_ID);

    pub fn export_screenshot(self, path: impl Into<PathBuf>, world: &World) -> bool {
        world
            .channels
            .render_tx
            .send(RenderMsg::CaptureTexture(self, path.into()))
            .is_ok()
    }
}

impl Texture2D {
    pub fn gen_fallback_diffuse(width: u32, height: u32) -> Vec<u8> {
        let mut diffuse = vec![];
        for x in 0..width as i32 {
            for y in 0..height as i32 {
                if x % 2 == y % 2 {
                    diffuse.extend_from_slice(&[0, 0, 0, 255]);
                } else {
                    diffuse.extend_from_slice(&[255, 0, 255, 255]);
                }
            }
        }
        diffuse
    }

    pub fn load_image(path: &str) -> Result<Texture2D, Box<dyn Error>> {
        let bytes = fs::read(path)?;
        Self::load_image_from_memory(&bytes)
    }

    pub fn load_image_from_memory(bytes: &[u8]) -> Result<Texture2D, Box<dyn Error>> {
        let image = image::load_from_memory(bytes)?;
        let rgba = image.into_rgba8();

        let mut data = Vec::with_capacity((rgba.width() * rgba.height() * 4) as usize);
        for pixel in rgba.pixels() {
            data.push(pixel[2]); // B
            data.push(pixel[1]); // G
            data.push(pixel[0]); // R
            data.push(pixel[3]); // A
        }

        Ok(Self::load_pixels(
            data,
            rgba.width(),
            rgba.height(),
            TextureFormat::Bgra8UnormSrgb,
        ))
    }

    pub fn load_pixels(
        pixels: Vec<u8>,
        width: u32,
        height: u32,
        format: TextureFormat,
    ) -> Texture2D {
        let has_transparency = Self::calculate_transparency(format, &pixels);
        Texture2D {
            width,
            height,
            format,
            data: Some(pixels),
            repeat_mode: AddressMode::Repeat,
            filter_mode: FilterMode::Linear,
            mip_filter_mode: MipmapFilterMode::Linear,
            has_transparency,
        }
    }

    pub fn load_pixels_with_transparency(
        pixels: Vec<u8>,
        width: u32,
        height: u32,
        format: TextureFormat,
        has_transparency: bool,
    ) -> Texture2D {
        Texture2D {
            width,
            height,
            format,
            data: Some(pixels),
            repeat_mode: AddressMode::Repeat,
            filter_mode: FilterMode::Linear,
            mip_filter_mode: MipmapFilterMode::Linear,
            has_transparency,
        }
    }

    pub fn calculate_transparency(format: TextureFormat, data: &[u8]) -> bool {
        let chunk_size = match format {
            TextureFormat::Rg8Unorm => 2,
            TextureFormat::Rgba8Unorm
            | TextureFormat::Rgba8UnormSrgb
            | TextureFormat::Rgba8Snorm
            | TextureFormat::Rgba8Uint
            | TextureFormat::Rgba8Sint
            | TextureFormat::Bgra8Unorm
            | TextureFormat::Bgra8UnormSrgb => 4,
            _ => return false,
        };

        for alpha in data.iter().skip(chunk_size - 1).step_by(chunk_size) {
            if *alpha < u8::MAX {
                return true;
            }
        }

        false
    }

    pub fn refresh_transparency(&mut self) {
        if let Some(data) = &self.data {
            self.has_transparency = Self::calculate_transparency(self.format, data);
        }
    }
}

impl StoreDefaults for Texture2D {
    fn populate(store: &mut Store<Self>) {
        const FALLBACK_SIZE: u32 = 35;

        store_add_checked!(
            store,
            HTexture2D::FALLBACK_DIFFUSE_ID,
            Texture2D::load_pixels_with_transparency(
                Self::gen_fallback_diffuse(FALLBACK_SIZE, FALLBACK_SIZE),
                FALLBACK_SIZE,
                FALLBACK_SIZE,
                TextureFormat::Bgra8UnormSrgb,
                false,
            )
        );

        store_add_checked!(
            store,
            HTexture2D::FALLBACK_NORMAL_ID,
            Texture2D::load_pixels_with_transparency(
                vec![0; 4],
                1,
                1,
                TextureFormat::Bgra8UnormSrgb,
                false
            )
        );

        store_add_checked!(
            store,
            HTexture2D::FALLBACK_SHININESS_ID,
            Texture2D::load_pixels_with_transparency(
                vec![0; 4],
                1,
                1,
                TextureFormat::Bgra8UnormSrgb,
                false
            )
        );
    }
}

impl StoreType for Texture2D {
    #[inline]
    fn name() -> &'static str {
        "Texture 2D"
    }

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        match handle.id() {
            HTexture2D::FALLBACK_DIFFUSE_ID => HandleName::Static("Diffuse Fallback"),
            HTexture2D::FALLBACK_NORMAL_ID => HandleName::Static("Normal Fallback"),
            HTexture2D::FALLBACK_SHININESS_ID => HandleName::Static("Diffuse Fallback"),
            _ => HandleName::Id(handle),
        }
    }

    fn is_builtin(handle: H<Self>) -> bool {
        handle.id() <= H::<Self>::MAX_BUILTIN_ID
    }
}

impl StoreTypeFallback for Texture2D {
    fn fallback() -> H<Self> {
        HTexture2D::FALLBACK_DIFFUSE
    }
}

impl Store<Texture2D> {}

impl TextureAsset for Texture2D {
    fn layer_count(&self) -> u32 {
        1
    }

    fn flags(&self) -> TextureUsages {
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_SRC | TextureUsages::COPY_DST
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn format(&self) -> TextureFormat {
        self.format
    }

    fn view_formats(&self) -> &[TextureFormat] {
        std::slice::from_ref(&self.format)
    }

    fn mip_level_count(&self) -> u32 {
        1
    }

    fn sample_count(&self) -> u32 {
        1
    }

    fn dimensions(&self) -> TextureDimension {
        TextureDimension::D2
    }

    fn view_dimension(&self) -> TextureViewDimension {
        TextureViewDimension::D2
    }

    fn repeat_mode(&self) -> AddressMode {
        self.repeat_mode
    }

    fn filter_mode(&self) -> FilterMode {
        self.filter_mode
    }

    fn mip_filter_mode(&self) -> MipmapFilterMode {
        self.mip_filter_mode
    }

    fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    fn has_transparency(&self) -> bool {
        self.has_transparency
    }
}
