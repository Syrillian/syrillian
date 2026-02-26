use crate::ShaderType;
use crate::mesh::Bones;
use crate::store::streaming::asset_store::{
    StreamingAssetBlobInfos, StreamingAssetBlobKind, StreamingAssetFile,
};
use crate::store::streaming::error::Result;
use glam::{Quat, Vec3, Vec4};
use serde_json::{Map, Value};
use snafu::{OptionExt, ResultExt, whatever};
use std::collections::HashMap;
use std::ops::Range;
use syrillian_utils::BoundingSphere;
use wgpu::{
    AddressMode, FilterMode, MipmapFilterMode, PolygonMode, PrimitiveTopology, TextureFormat,
};

pub trait MapDecodeHelper {
    fn required_field(&self, field: &str) -> Result<&Value>;
    fn optional_field(&self, field: &str) -> Option<&Value>;
}

pub trait ParseDecode<T> {
    fn expect_parse(&self, label: &str) -> Result<T>;
}

pub trait ParseDecodeWithBlobs<T> {
    fn expect_parse_blobs(
        &self,
        blobs: &StreamingAssetBlobInfos,
        package: &mut StreamingAssetFile,
    ) -> Result<T>;
}

pub trait DecodeHelper {
    fn expect_object(&self, label: &str) -> Result<&Map<String, Value>>;
    fn expect_array(&self, label: &str) -> Result<&[Value]>;
    fn as_optional_f32(&self, label: &str) -> Result<Option<f32>>;
    fn expect_u64(&self, label: &str) -> Result<u64>;
    fn expect_u32(&self, label: &str) -> Result<u32>;
    fn expect_usize(&self, label: &str) -> Result<usize>;
    fn expect_f32(&self, label: &str) -> Result<f32>;
    fn expect_bool(&self, label: &str) -> Result<bool>;
    fn expect_str(&self, label: &str) -> Result<&str>;
}

impl MapDecodeHelper for Map<String, Value> {
    fn required_field(&self, field: &str) -> Result<&Value> {
        self.get(field)
            .with_whatever_context(|| format!("required {field} not found"))
    }

    fn optional_field(&self, field: &str) -> Option<&Value> {
        self.get(field)
    }
}

impl DecodeHelper for Value {
    fn expect_object(&self, label: &str) -> Result<&Map<String, Value>> {
        self.as_object()
            .with_whatever_context(|| format!("expected json object for {label}"))
    }

    fn expect_array(&self, label: &str) -> Result<&[Value]> {
        self.as_array()
            .map(Vec::as_slice)
            .with_whatever_context(|| format!("expected json array for {label}"))
    }

    fn as_optional_f32(&self, label: &str) -> Result<Option<f32>> {
        if self.is_null() {
            return Ok(None);
        }
        Ok(Some(self.expect_f32(label)?))
    }

    fn expect_u64(&self, label: &str) -> Result<u64> {
        self.expect_parse(label)
    }

    fn expect_u32(&self, label: &str) -> Result<u32> {
        self.expect_parse(label)
    }

    fn expect_usize(&self, label: &str) -> Result<usize> {
        self.expect_parse(label)
    }

    fn expect_f32(&self, label: &str) -> Result<f32> {
        self.expect_parse(label)
    }

    fn expect_bool(&self, label: &str) -> Result<bool> {
        self.expect_parse(label)
    }

    fn expect_str(&self, label: &str) -> Result<&str> {
        self.as_str()
            .with_whatever_context(|| format!("{label} must be a string"))
    }
}

impl ParseDecode<u64> for Value {
    fn expect_parse(&self, label: &str) -> Result<u64> {
        if let Some(value) = self.as_u64() {
            return Ok(value);
        }
        if let Some(value) = self.as_i64()
            && value >= 0
        {
            return Ok(value as u64);
        }
        whatever!("{label} must be an unsigned integer")
    }
}

impl ParseDecode<u32> for Value {
    fn expect_parse(&self, label: &str) -> Result<u32> {
        let value = self.expect_u64(label)?;
        u32::try_from(value)
            .with_whatever_context(|_| format!("{label} value {value} does not fit into u32"))
    }
}

impl ParseDecode<usize> for Value {
    fn expect_parse(&self, label: &str) -> Result<usize> {
        let value = self.expect_u64(label)?;
        usize::try_from(value)
            .with_whatever_context(|_| format!("{label} value {value} does not fit into usize"))
    }
}

impl ParseDecode<f32> for Value {
    fn expect_parse(&self, label: &str) -> Result<f32> {
        let Some(value) = self.as_f64() else {
            whatever!("{label} must be a number");
        };
        Ok(value as f32)
    }
}

impl ParseDecode<bool> for Value {
    fn expect_parse(&self, label: &str) -> Result<bool> {
        self.as_bool()
            .with_whatever_context(|| format!("{label} must be a boolean"))
    }
}

impl ParseDecode<Range<u32>> for Value {
    fn expect_parse(&self, label: &str) -> Result<Range<u32>> {
        let object = self.expect_object(label)?;
        let start = object.required_field("start")?.expect_u32(label)?;
        let end = object.required_field("end")?.expect_u32(label)?;
        Ok(start..end)
    }
}

impl ParseDecode<String> for Value {
    fn expect_parse(&self, label: &str) -> Result<String> {
        Ok(self.expect_str(label)?.to_string())
    }
}

impl<T> ParseDecode<Vec<T>> for Value
where
    Self: ParseDecode<T>,
{
    fn expect_parse(&self, label: &str) -> Result<Vec<T>> {
        let array = self.expect_array(label)?;
        let mut values = Vec::with_capacity(array.len());
        for item in array {
            let obj = item.expect_parse(label)?;
            values.push(obj);
        }
        Ok(values)
    }
}

impl<V> ParseDecode<HashMap<String, V>> for Value
where
    Self: ParseDecode<V>,
{
    fn expect_parse(&self, label: &str) -> Result<HashMap<String, V>> {
        let object = self.expect_object(label)?;
        let mut out = HashMap::with_capacity(object.len());
        for (key, value) in object {
            out.insert(key.clone(), value.expect_parse(label)?);
        }
        Ok(out)
    }
}

impl<T> ParseDecode<Option<T>> for Value
where
    Self: ParseDecode<T>,
{
    fn expect_parse(&self, label: &str) -> Result<Option<T>> {
        if self.is_null() {
            Ok(None)
        } else {
            self.expect_parse(label).map(Some)
        }
    }
}
impl<T, P> ParseDecode<Option<T>> for Option<&P>
where
    P: ParseDecode<T>,
{
    fn expect_parse(&self, label: &str) -> Result<Option<T>> {
        match self {
            None => Ok(None),
            Some(this) => this.expect_parse(label).map(Some),
        }
    }
}

impl ParseDecode<Vec3> for Value {
    fn expect_parse(&self, label: &str) -> Result<Vec3> {
        let array = self.expect_array(label)?;
        if array.len() != 3 {
            whatever!("{label} expected 3 elements but found {}", array.len());
        }
        Ok(Vec3::new(
            array[0].expect_f32(label)?,
            array[1].expect_f32(label)?,
            array[2].expect_f32(label)?,
        ))
    }
}

impl ParseDecode<Vec4> for Value {
    fn expect_parse(&self, label: &str) -> Result<Vec4> {
        let array = self.expect_array(label)?;
        if array.len() != 4 {
            whatever!("{label} expected 4 elements but found {}", array.len());
        }
        Ok(Vec4::new(
            array[0].expect_f32(label)?,
            array[1].expect_f32(label)?,
            array[2].expect_f32(label)?,
            array[3].expect_f32(label)?,
        ))
    }
}

impl ParseDecode<Quat> for Value {
    fn expect_parse(&self, label: &str) -> Result<Quat> {
        let vec = self.expect_parse(label)?;
        Ok(Quat::from_vec4(vec))
    }
}

impl ParseDecode<BoundingSphere> for Value {
    fn expect_parse(&self, label: &str) -> Result<BoundingSphere> {
        let bounds = self.expect_object(label)?;
        let center = bounds
            .required_field("center")?
            .expect_parse("mesh bounding sphere center")?;
        let radius = bounds
            .required_field("radius")?
            .expect_f32("mesh bounding sphere radius")?;

        Ok(BoundingSphere { center, radius })
    }
}

impl ParseDecode<TextureFormat> for Value {
    fn expect_parse(&self, label: &str) -> Result<TextureFormat> {
        let name = self.expect_str(label)?;
        match name {
            "R8Unorm" => Ok(TextureFormat::R8Unorm),
            "Rg8Unorm" => Ok(TextureFormat::Rg8Unorm),
            "Rgba8Unorm" => Ok(TextureFormat::Rgba8Unorm),
            "Rgba8UnormSrgb" => Ok(TextureFormat::Rgba8UnormSrgb),
            "Bgra8Unorm" => Ok(TextureFormat::Bgra8Unorm),
            "Bgra8UnormSrgb" => Ok(TextureFormat::Bgra8UnormSrgb),
            "Bc1RgbaUnorm" => Ok(TextureFormat::Bc1RgbaUnorm),
            "Bc1RgbaUnormSrgb" => Ok(TextureFormat::Bc1RgbaUnormSrgb),
            "Bc3RgbaUnorm" => Ok(TextureFormat::Bc3RgbaUnorm),
            "Bc3RgbaUnormSrgb" => Ok(TextureFormat::Bc3RgbaUnormSrgb),
            "R16Unorm" => Ok(TextureFormat::R16Unorm),
            "Rg16Snorm" => Ok(TextureFormat::Rg16Snorm),
            "Rgba16Unorm" => Ok(TextureFormat::Rgba16Unorm),
            "Rgba32Float" => Ok(TextureFormat::Rgba32Float),
            other => whatever!("unsupported texture format '{other}'"),
        }
    }
}

impl ParseDecode<AddressMode> for Value {
    fn expect_parse(&self, label: &str) -> Result<AddressMode> {
        let name = self.expect_str(label)?;
        match name {
            "ClampToEdge" => Ok(AddressMode::ClampToEdge),
            "Repeat" => Ok(AddressMode::Repeat),
            "MirrorRepeat" => Ok(AddressMode::MirrorRepeat),
            "ClampToBorder" => Ok(AddressMode::ClampToBorder),
            other => whatever!("unsupported texture repeat mode '{other}'"),
        }
    }
}

impl ParseDecode<FilterMode> for Value {
    fn expect_parse(&self, label: &str) -> Result<FilterMode> {
        let name = self.expect_str(label)?;
        match name {
            "Nearest" => Ok(FilterMode::Nearest),
            "Linear" => Ok(FilterMode::Linear),
            other => whatever!("unsupported texture filter mode '{other}'"),
        }
    }
}

impl ParseDecode<MipmapFilterMode> for Value {
    fn expect_parse(&self, label: &str) -> Result<MipmapFilterMode> {
        let name = self.expect_str(label)?;
        match name {
            "Nearest" => Ok(MipmapFilterMode::Nearest),
            "Linear" => Ok(MipmapFilterMode::Linear),
            other => whatever!("unsupported mip filter mode '{other}'"),
        }
    }
}

impl ParseDecode<PolygonMode> for Value {
    fn expect_parse(&self, label: &str) -> Result<PolygonMode> {
        let name = self.expect_str(label)?;
        match name {
            "Fill" => Ok(PolygonMode::Fill),
            "Line" => Ok(PolygonMode::Line),
            "Point" => Ok(PolygonMode::Point),
            other => whatever!("unsupported shader polygon mode '{other}'"),
        }
    }
}

impl ParseDecode<PrimitiveTopology> for Value {
    fn expect_parse(&self, label: &str) -> Result<PrimitiveTopology> {
        let name = self.expect_str(label)?;
        match name {
            "PointList" => Ok(PrimitiveTopology::PointList),
            "LineList" => Ok(PrimitiveTopology::LineList),
            "LineStrip" => Ok(PrimitiveTopology::LineStrip),
            "TriangleList" => Ok(PrimitiveTopology::TriangleList),
            "TriangleStrip" => Ok(PrimitiveTopology::TriangleStrip),
            other => whatever!("unsupported shader topology '{other}'"),
        }
    }
}

impl ParseDecode<ShaderType> for Value {
    fn expect_parse(&self, label: &str) -> Result<ShaderType> {
        let name = self.expect_str(label)?;
        match name {
            "Default" => Ok(ShaderType::Default),
            "Custom" => Ok(ShaderType::Custom),
            "PostProcessing" => Ok(ShaderType::PostProcessing),
            other => whatever!("unsupported shader type '{other}'"),
        }
    }
}

impl ParseDecodeWithBlobs<Bones> for Value {
    fn expect_parse_blobs(
        &self,
        blobs: &StreamingAssetBlobInfos,
        package: &mut StreamingAssetFile,
    ) -> Result<Bones> {
        let bones = self.expect_object("mesh bones")?;
        let names = bones
            .required_field("names")?
            .expect_parse("mesh bone names")?;
        let parents = bones
            .required_field("parents")?
            .expect_parse("mesh bone parents")?;
        let children = bones
            .required_field("children")?
            .expect_parse("mesh bone children")?;
        let roots = bones
            .required_field("roots")?
            .expect_parse("mesh bone roots")?;
        let index_of = bones
            .required_field("index_of")?
            .expect_parse("mesh bone index map")?;

        let inverse_bind = blobs
            .find(StreamingAssetBlobKind::BonesInverseBind)?
            .decode_all_from_io(package)?;

        let bind_global = blobs
            .find(StreamingAssetBlobKind::BonesBindGlobal)?
            .decode_all_from_io(package)?;

        let bind_local = blobs
            .find(StreamingAssetBlobKind::BonesBindLocal)?
            .decode_all_from_io(package)?;

        Ok(Bones {
            names,
            parents,
            children,
            roots,
            inverse_bind,
            bind_global,
            bind_local,
            index_of,
        })
    }
}
