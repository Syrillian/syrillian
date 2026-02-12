use crate::store::{H, HandleName, Store, StoreDefaults, StoreType, StoreTypeFallback};
use crate::{HTexture2D, store_add_checked};
use std::error::Error;
use std::fs;
use wgpu::{AddressMode, FilterMode, MipmapFilterMode, TextureFormat};

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
    const NAME: &str = "Texture 2D";

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
