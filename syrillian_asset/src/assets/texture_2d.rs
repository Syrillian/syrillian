use crate::store::streaming::asset_store::{
    StreamingAssetBlobKind, StreamingAssetFile, StreamingAssetPayload,
};
use crate::store::streaming::decode_helper::{DecodeHelper, MapDecodeHelper, ParseDecode};
use crate::store::streaming::packaged_scene::{BuiltPayload, PackedBlob};
use crate::store::streaming::payload::StreamableAsset;
use crate::store::{
    H, HandleName, Store, StoreDefaults, StoreType, StoreTypeFallback, UpdateAssetMessage,
    streaming,
};
use crate::{HTexture2D, store_add_checked};
use crossbeam_channel::Sender;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use syrillian_reflect::serializer::JsonSerializer;
use syrillian_reflect::{ReflectSerialize, Value};
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

    fn refresh_dirty(
        &self,
        key: crate::store::AssetKey,
        assets_tx: &Sender<(crate::store::AssetKey, UpdateAssetMessage)>,
    ) -> bool {
        assets_tx
            .send((key, UpdateAssetMessage::UpdateTexture2D(self.clone())))
            .is_ok()
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

struct TextureMeta<'a> {
    texture: &'a Texture2D,
}

impl ReflectSerialize for TextureMeta<'_> {
    fn serialize(this: &Self) -> Value {
        let data_len = this.texture.data.as_ref().map_or(0, |data| data.len()) as u64;

        Value::Object(BTreeMap::from([
            ("width".to_string(), Value::UInt(this.texture.width)),
            ("height".to_string(), Value::UInt(this.texture.height)),
            (
                "format".to_string(),
                Value::String(format!("{:?}", this.texture.format)),
            ),
            (
                "repeat_mode".to_string(),
                Value::String(format!("{:?}", this.texture.repeat_mode)),
            ),
            (
                "filter_mode".to_string(),
                Value::String(format!("{:?}", this.texture.filter_mode)),
            ),
            (
                "mip_filter_mode".to_string(),
                Value::String(format!("{:?}", this.texture.mip_filter_mode)),
            ),
            (
                "has_transparency".to_string(),
                Value::Bool(this.texture.has_transparency),
            ),
            (
                "has_data".to_string(),
                Value::Bool(this.texture.data.is_some()),
            ),
            ("data_len".to_string(), Value::BigUInt(data_len)),
            ("data".to_string(), Value::None),
        ]))
    }
}

impl StreamableAsset for Texture2D {
    fn encode(&self) -> BuiltPayload {
        let mut blobs = Vec::new();
        if let Some(data) = self.data.as_deref()
            && !data.is_empty()
        {
            blobs.push(PackedBlob {
                kind: StreamingAssetBlobKind::TextureData,
                element_count: data.len() as u64,
                data: data.to_vec(),
            });
        }

        BuiltPayload {
            payload: JsonSerializer::serialize_to_string(&TextureMeta { texture: self }),
            blobs,
        }
    }

    fn decode(
        payload: &StreamingAssetPayload,
        package: &mut StreamingAssetFile,
    ) -> streaming::error::Result<Self> {
        let root = payload.data.expect_object("texture metadata root")?;

        let width = root
            .required_field("width")?
            .expect_parse("texture width")?;
        let height = root
            .required_field("height")?
            .expect_parse("texture height")?;
        let format = root
            .required_field("format")?
            .expect_parse("texture format")?;
        let repeat_mode = root
            .required_field("repeat_mode")?
            .expect_parse("texture repeat mode")?;
        let filter_mode = root
            .required_field("filter_mode")?
            .expect_parse("texture filter mode")?;
        let mip_filter_mode = root
            .required_field("mip_filter_mode")?
            .expect_parse("texture mip filter mode")?;
        let has_transparency = root
            .required_field("has_transparency")?
            .expect_parse("texture has_transparency")?;
        let has_data = root
            .required_field("has_data")?
            .expect_parse("texture has_data")?;
        let data_len: usize = root
            .optional_field("data_len")
            .expect_parse("texture data_len")?
            .unwrap_or(0usize);

        let data = if has_data {
            let blob = payload
                .blob_infos
                .find(StreamingAssetBlobKind::TextureData)?;
            let data_len = if data_len == 0 {
                blob.element_count
            } else {
                data_len
            };
            Some(blob.decode_from_io("texture data", data_len, package)?)
        } else {
            None
        };

        Ok(Texture2D {
            width,
            height,
            format,
            data,
            repeat_mode,
            filter_mode,
            mip_filter_mode,
            has_transparency,
        })
    }
}

impl Store<Texture2D> {}
