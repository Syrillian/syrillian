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
use crate::{HCubemap, store_add_checked};
use crossbeam_channel::Sender;
use std::collections::BTreeMap;
use std::error::Error;
use std::f32::consts::PI;
use std::path::Path;
use syrillian_reflect::serializer::JsonSerializer;
use syrillian_reflect::{ReflectSerialize, Value};
use wgpu::{AddressMode, FilterMode, MipmapFilterMode, TextureFormat};

#[derive(Debug, Clone)]
pub struct Cubemap {
    pub width: u32,
    pub height: u32,
    pub mip_level_count: u32,
    pub format: TextureFormat,
    pub data: Option<Vec<u8>>,
    pub repeat_mode: AddressMode,
    pub filter_mode: FilterMode,
    pub mip_filter_mode: MipmapFilterMode,
    pub has_transparency: bool,
}

impl H<Cubemap> {
    pub const FALLBACK_ID: u32 = 0;
    pub const MAX_BUILTIN_ID: u32 = 0;

    pub const FALLBACK: H<Cubemap> = H::new(Self::FALLBACK_ID);
}

impl Cubemap {
    pub fn fallback() -> Self {
        let mut data = Vec::with_capacity(6 * 4);
        for _ in 0..6 {
            data.extend_from_slice(&[110, 150, 220, 255]);
        }

        Self {
            width: 1,
            height: 1,
            mip_level_count: 1,
            format: TextureFormat::Rgba8UnormSrgb,
            data: Some(data),
            repeat_mode: AddressMode::ClampToEdge,
            filter_mode: FilterMode::Linear,
            mip_filter_mode: MipmapFilterMode::Linear,
            has_transparency: false,
        }
    }

    pub fn load_equirect_hdr<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let bytes = std::fs::read(path)?;
        Self::load_equirect_hdr_from_memory(&bytes)
    }

    pub fn load_equirect_hdr_from_memory(bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        let image = image::load_from_memory(bytes)?;
        Ok(Self::from_equirect_rgb32f(&image.into_rgb32f()))
    }

    fn from_equirect_rgb32f(image: &image::Rgb32FImage) -> Self {
        let src_w = image.width().max(1);
        let src_h = image.height().max(1);
        let face = (src_w / 4).max(1).min((src_h / 2).max(1));
        let mip_level_count = max_mip_levels(face, face).min(3);

        let mut base_faces = Vec::with_capacity(6);

        for face_idx in 0..6u32 {
            let mut face_data = Vec::with_capacity((face * face * 4) as usize);
            for y in 0..face {
                let v = 2.0 * ((y as f32 + 0.5) / face as f32) - 1.0;
                for x in 0..face {
                    let u = 2.0 * ((x as f32 + 0.5) / face as f32) - 1.0;
                    let dir = cubemap_face_dir(face_idx, u, v);

                    let theta = dir[2].atan2(dir[0]);
                    let phi = dir[1].clamp(-1.0, 1.0).acos();
                    let u_eq = (theta + PI) / (2.0 * PI);
                    let v_eq = phi / PI;

                    let sx = (u_eq * (src_w as f32 - 1.0))
                        .round()
                        .clamp(0.0, src_w as f32 - 1.0) as u32;
                    let sy = (v_eq * (src_h as f32 - 1.0))
                        .round()
                        .clamp(0.0, src_h as f32 - 1.0) as u32;

                    let px = image.get_pixel(sx, sy).0;

                    let r = tonemap_to_srgb_u8(px[0]);
                    let g = tonemap_to_srgb_u8(px[1]);
                    let b = tonemap_to_srgb_u8(px[2]);
                    face_data.extend_from_slice(&[r, g, b, 255]);
                }
            }
            base_faces.push(face_data);
        }

        let mut data = Vec::with_capacity(total_mip_byte_size(face, mip_level_count));

        for base_face in &base_faces {
            let mut prev = base_face.clone();
            let mut prev_size = face;
            data.extend_from_slice(&prev);

            for _ in 1..mip_level_count {
                let next_size = (prev_size / 2).max(1);
                let next = downsample_face_rgba8_srgb(&prev, prev_size, next_size);
                data.extend_from_slice(&next);
                prev = next;
                prev_size = next_size;
            }
        }

        Self {
            width: face,
            height: face,
            mip_level_count,
            format: TextureFormat::Rgba8UnormSrgb,
            data: Some(data),
            repeat_mode: AddressMode::ClampToEdge,
            filter_mode: FilterMode::Linear,
            mip_filter_mode: MipmapFilterMode::Linear,
            has_transparency: false,
        }
    }
}

impl StoreDefaults for Cubemap {
    fn populate(store: &mut Store<Self>) {
        store_add_checked!(store, HCubemap::FALLBACK_ID, Cubemap::fallback());
    }
}

impl StoreType for Cubemap {
    const NAME: &str = "Cubemap";

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        match handle.id() {
            HCubemap::FALLBACK_ID => HandleName::Static("Cubemap Fallback"),
            _ => HandleName::Id(handle),
        }
    }

    fn refresh_dirty(
        &self,
        key: crate::store::AssetKey,
        assets_tx: &Sender<(crate::store::AssetKey, UpdateAssetMessage)>,
    ) -> bool {
        assets_tx
            .send((key, UpdateAssetMessage::UpdateCubemap(self.clone())))
            .is_ok()
    }

    fn is_builtin(handle: H<Self>) -> bool {
        handle.id() == HCubemap::MAX_BUILTIN_ID
    }
}

impl StoreTypeFallback for Cubemap {
    fn fallback() -> H<Self> {
        HCubemap::FALLBACK
    }
}

struct CubemapMeta<'a> {
    cubemap: &'a Cubemap,
}

impl<'a> ReflectSerialize for CubemapMeta<'a> {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            ("width".to_string(), Value::UInt(this.cubemap.width)),
            ("height".to_string(), Value::UInt(this.cubemap.height)),
            (
                "mip_level_count".to_string(),
                Value::UInt(this.cubemap.mip_level_count),
            ),
            (
                "format".to_string(),
                Value::String(format!("{:?}", this.cubemap.format)),
            ),
            (
                "repeat_mode".to_string(),
                Value::String(format!("{:?}", this.cubemap.repeat_mode)),
            ),
            (
                "filter_mode".to_string(),
                Value::String(format!("{:?}", this.cubemap.filter_mode)),
            ),
            (
                "mip_filter_mode".to_string(),
                Value::String(format!("{:?}", this.cubemap.mip_filter_mode)),
            ),
            (
                "has_transparency".to_string(),
                Value::Bool(this.cubemap.has_transparency),
            ),
        ]))
    }
}

impl StreamableAsset for Cubemap {
    fn encode(&self) -> BuiltPayload {
        let mut blobs = Vec::new();

        if let Some(data) = self.data.as_deref() {
            PackedBlob::maybe_pack_data_into(
                StreamingAssetBlobKind::TextureData,
                &data,
                &mut blobs,
            );
        }

        BuiltPayload {
            payload: JsonSerializer::serialize_to_string(&CubemapMeta { cubemap: self }),
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
        let mip_level_count = root
            .required_field("mip_level_count")?
            .expect_parse("mip level count")?;
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

        let data = payload
            .blob_infos
            .find(StreamingAssetBlobKind::TextureData)
            .ok()
            .map(|b| b.decode_all_from_io(package))
            .transpose()?;

        Ok(Cubemap {
            width,
            height,
            mip_level_count,
            format,
            data,
            repeat_mode,
            filter_mode,
            mip_filter_mode,
            has_transparency,
        })
    }
}

fn cubemap_face_dir(face: u32, u: f32, v: f32) -> [f32; 3] {
    let (x, y, z) = match face {
        0 => (1.0, -v, -u),  // +X
        1 => (-1.0, -v, u),  // -X
        2 => (u, 1.0, v),    // +Y
        3 => (u, -1.0, -v),  // -Y
        4 => (u, -v, 1.0),   // +Z
        _ => (-u, -v, -1.0), // -Z
    };

    let len = (x * x + y * y + z * z).sqrt().max(1e-6);
    [x / len, y / len, z / len]
}

fn max_mip_levels(width: u32, height: u32) -> u32 {
    let mut levels = 1;
    let mut w = width.max(1);
    let mut h = height.max(1);

    while w > 1 || h > 1 {
        w = (w / 2).max(1);
        h = (h / 2).max(1);
        levels += 1;
    }

    levels
}

fn total_mip_byte_size(base_size: u32, mip_levels: u32) -> usize {
    let mut total = 0usize;
    for level in 0..mip_levels {
        let size = (base_size >> level).max(1) as usize;
        total += size * size * 4 * 6;
    }
    total
}

fn downsample_face_rgba8_srgb(src: &[u8], src_size: u32, dst_size: u32) -> Vec<u8> {
    let mut out = vec![0u8; (dst_size * dst_size * 4) as usize];

    for y in 0..dst_size {
        for x in 0..dst_size {
            let sx = x * 2;
            let sy = y * 2;
            let sample_coords = [
                (sx, sy),
                ((sx + 1).min(src_size - 1), sy),
                (sx, (sy + 1).min(src_size - 1)),
                ((sx + 1).min(src_size - 1), (sy + 1).min(src_size - 1)),
            ];

            let mut accum_r = 0.0f32;
            let mut accum_g = 0.0f32;
            let mut accum_b = 0.0f32;
            let mut accum_a = 0u32;

            for (px, py) in sample_coords {
                let i = ((py * src_size + px) * 4) as usize;
                accum_r += srgb_u8_to_linear(src[i]);
                accum_g += srgb_u8_to_linear(src[i + 1]);
                accum_b += srgb_u8_to_linear(src[i + 2]);
                accum_a += src[i + 3] as u32;
            }

            let di = ((y * dst_size + x) * 4) as usize;
            out[di] = linear_to_srgb_u8(accum_r * 0.25);
            out[di + 1] = linear_to_srgb_u8(accum_g * 0.25);
            out[di + 2] = linear_to_srgb_u8(accum_b * 0.25);
            out[di + 3] = ((accum_a + 2) / 4) as u8;
        }
    }

    out
}

fn srgb_u8_to_linear(v: u8) -> f32 {
    let s = (v as f32) * (1.0 / 255.0);
    s.powf(2.2)
}

fn linear_to_srgb_u8(v: f32) -> u8 {
    let s = v.max(0.0).powf(1.0 / 2.2);
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

fn tonemap_to_srgb_u8(v: f32) -> u8 {
    let v = v.max(0.0);
    let mapped = v / (1.0 + v);
    let srgb = mapped.powf(1.0 / 2.2);
    (srgb * 255.0).round().clamp(0.0, 255.0) as u8
}
