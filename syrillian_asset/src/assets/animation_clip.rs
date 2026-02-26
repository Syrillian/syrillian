use crate::store::streaming::asset_store::{
    StreamingAssetBlobInfo, StreamingAssetBlobInfos, StreamingAssetBlobKind, StreamingAssetFile,
    StreamingAssetPayload,
};
use crate::store::streaming::decode_helper::{DecodeHelper, MapDecodeHelper, ParseDecode};
use crate::store::streaming::error::BlobNotFoundErr;
use crate::store::streaming::packaged_scene::{BuiltPayload, PackedBlob};
use crate::store::streaming::payload::StreamableAsset;
use crate::store::{H, HandleName, StoreType, UpdateAssetMessage, streaming};
use crossbeam_channel::Sender;
use glamx::{EulerRot, Quat, Vec3};
use serde_json::Value as JsonValue;
use snafu::{OptionExt, ensure_whatever, whatever};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use syrillian_reflect::serializer::JsonSerializer;
use syrillian_reflect::{ReflectSerialize, Value};
use zerocopy::IntoBytes;

pub type HAnimationClipAsset = H<AnimationClip>;

#[derive(Debug, Clone, Default)]
pub struct AnimationClip {
    pub name: String,
    pub duration: f32,
    pub channels: Vec<AnimationChannel>,
}

#[derive(Debug, Clone, Default)]
pub struct AnimationChannel {
    pub target_name: String,
    pub keys: TransformKeys,
}

/// Per-node keyframes.
/// Times are in **seconds**.
#[derive(Debug, Clone, Default)]
pub struct TransformKeys {
    pub t_times: Vec<f32>,
    pub t_values: Vec<Vec3>,

    pub r_times: Vec<f32>,
    pub r_values: Vec<Quat>,

    pub s_times: Vec<f32>,
    pub s_values: Vec<Vec3>,
}

impl StoreType for AnimationClip {
    const NAME: &str = "AnimationClip";

    fn ident_fmt(handle: H<Self>) -> HandleName<Self> {
        HandleName::Id(handle)
    }

    fn refresh_dirty(
        &self,
        _key: crate::store::AssetKey,
        _assets_tx: &Sender<(crate::store::AssetKey, UpdateAssetMessage)>,
    ) -> bool {
        false
    }

    fn is_builtin(_handle: H<Self>) -> bool {
        false
    }
}

impl TransformKeys {
    pub fn keyed_translation(times: &[f32], values: &[[f32; 3]]) -> Self {
        Self {
            t_times: times.to_vec(),
            t_values: values.iter().copied().map(Vec3::from).collect(),
            ..Default::default()
        }
    }

    pub fn keyed_scale(times: &[f32], values: &[[f32; 3]]) -> Self {
        Self {
            s_times: times.to_vec(),
            s_values: values.iter().copied().map(Vec3::from).collect(),
            ..Default::default()
        }
    }

    pub fn keyed_rotation(times: &[f32], angles: &[f32]) -> Self {
        Self {
            r_times: times.to_vec(),
            r_values: angles
                .iter()
                .map(|a| Quat::from_euler(EulerRot::XYZ, 0.0, 0.0, *a))
                .collect(),
            ..Default::default()
        }
    }

    pub fn sample(&self, t: f32) -> (Option<Vec3>, Option<Quat>, Option<Vec3>) {
        (
            self.sample_translation(t),
            self.sample_rotation(t),
            self.sample_scale(t),
        )
    }

    pub fn sample_translation(&self, t: f32) -> Option<Vec3> {
        let n = self.t_times.len();
        if n == 0 {
            return None;
        }
        if n == 1 {
            return Some(self.t_values[0]);
        }

        let i = Self::find_key(&self.t_times, t);
        if i == n - 1 {
            return Some(self.t_values[i]);
        }
        let t0 = self.t_times[i];
        let t1 = self.t_times[i + 1];
        let a = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
        Some(self.t_values[i].lerp(self.t_values[i + 1], a))
    }

    pub fn sample_scale(&self, t: f32) -> Option<Vec3> {
        let n = self.s_times.len();
        if n == 0 {
            return None;
        }
        if n == 1 {
            return Some(self.s_values[0]);
        }

        let i = Self::find_key(&self.s_times, t);
        if i == n - 1 {
            return Some(self.s_values[i]);
        }
        let t0 = self.s_times[i];
        let t1 = self.s_times[i + 1];
        let a = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
        Some(self.s_values[i].lerp(self.s_values[i + 1], a))
    }

    pub fn sample_rotation(&self, t: f32) -> Option<Quat> {
        let n = self.r_times.len();
        if n == 0 {
            return None;
        }
        if n == 1 {
            return Some(self.r_values[0]);
        }

        let i = Self::find_key(&self.r_times, t);
        if i == n - 1 {
            return Some(self.r_values[i]);
        }
        let t0 = self.r_times[i];
        let t1 = self.r_times[i + 1];
        let a = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
        Some(self.r_values[i].slerp(self.r_values[i + 1], a).normalize())
    }

    fn find_key(times: &[f32], t: f32) -> usize {
        if times.is_empty() {
            return 0;
        }
        if t <= times[0] {
            return 0;
        }
        if t >= *times.last().unwrap() {
            return times.len() - 1;
        }
        times
            .binary_search_by(|k| k.partial_cmp(&t).unwrap_or(Ordering::Equal))
            .unwrap_or_else(|i| (i - 1).max(0))
    }
}

struct AnimationClipMeta<'a> {
    clip: &'a AnimationClip,
}

struct AnimationChannelMeta<'a> {
    channel: &'a AnimationChannel,
}

struct AnimationKeysMeta<'a> {
    keys: &'a TransformKeys,
}

impl ReflectSerialize for AnimationKeysMeta<'_> {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "t_times_count".to_string(),
                Value::BigUInt(this.keys.t_times.len() as u64),
            ),
            (
                "t_values_count".to_string(),
                Value::BigUInt(this.keys.t_values.len() as u64),
            ),
            (
                "r_times_count".to_string(),
                Value::BigUInt(this.keys.r_times.len() as u64),
            ),
            (
                "r_values_count".to_string(),
                Value::BigUInt(this.keys.r_values.len() as u64),
            ),
            (
                "s_times_count".to_string(),
                Value::BigUInt(this.keys.s_times.len() as u64),
            ),
            (
                "s_values_count".to_string(),
                Value::BigUInt(this.keys.s_values.len() as u64),
            ),
        ]))
    }
}

impl ReflectSerialize for AnimationChannelMeta<'_> {
    fn serialize(this: &Self) -> Value {
        Value::Object(BTreeMap::from([
            (
                "target_name".to_string(),
                Value::String(this.channel.target_name.clone()),
            ),
            (
                "keys".to_string(),
                ReflectSerialize::serialize(&AnimationKeysMeta {
                    keys: &this.channel.keys,
                }),
            ),
        ]))
    }
}

impl ReflectSerialize for AnimationClipMeta<'_> {
    fn serialize(this: &Self) -> Value {
        let channels = this
            .clip
            .channels
            .iter()
            .map(|channel| ReflectSerialize::serialize(&AnimationChannelMeta { channel }))
            .collect::<Vec<_>>();

        Value::Object(BTreeMap::from([
            ("name".to_string(), Value::String(this.clip.name.clone())),
            ("duration".to_string(), Value::Float(this.clip.duration)),
            (
                "channel_count".to_string(),
                Value::BigUInt(channels.len() as u64),
            ),
            ("channels".to_string(), Value::Array(channels)),
        ]))
    }
}

impl StreamableAsset for AnimationClip {
    fn encode(&self) -> BuiltPayload {
        let mut blobs = Vec::new();

        for channel in &self.channels {
            let keys = &channel.keys;

            let t_times_blob = keys.t_times.as_bytes();
            if !t_times_blob.is_empty() {
                blobs.push(PackedBlob {
                    kind: StreamingAssetBlobKind::AnimationTranslationTimes,
                    element_count: keys.t_times.len() as u64,
                    data: t_times_blob.to_vec(),
                });
            }

            let t_values_blob = keys.t_values.as_bytes();
            if !t_values_blob.is_empty() {
                blobs.push(PackedBlob {
                    kind: StreamingAssetBlobKind::AnimationTranslationValues,
                    element_count: keys.t_values.len() as u64,
                    data: t_values_blob.to_vec(),
                });
            }

            let r_times_blob = keys.r_times.as_bytes();
            if !r_times_blob.is_empty() {
                blobs.push(PackedBlob {
                    kind: StreamingAssetBlobKind::AnimationRotationTimes,
                    element_count: keys.r_times.len() as u64,
                    data: r_times_blob.to_vec(),
                });
            }

            let r_values_blob = keys.r_values.as_bytes();
            if !r_values_blob.is_empty() {
                blobs.push(PackedBlob {
                    kind: StreamingAssetBlobKind::AnimationRotationValues,
                    element_count: keys.r_values.len() as u64,
                    data: r_values_blob.to_vec(),
                });
            }

            let s_times_blob = keys.s_times.as_bytes();
            if !s_times_blob.is_empty() {
                blobs.push(PackedBlob {
                    kind: StreamingAssetBlobKind::AnimationScaleTimes,
                    element_count: keys.s_times.len() as u64,
                    data: s_times_blob.to_vec(),
                });
            }

            let s_values_blob = keys.s_values.as_bytes();
            if !s_values_blob.is_empty() {
                blobs.push(PackedBlob {
                    kind: StreamingAssetBlobKind::AnimationScaleValues,
                    element_count: keys.s_values.len() as u64,
                    data: s_values_blob.to_vec(),
                });
            }
        }

        BuiltPayload {
            payload: JsonSerializer::serialize_to_string(&AnimationClipMeta { clip: self }),
            blobs,
        }
    }

    fn decode(
        payload: &StreamingAssetPayload,
        package: &mut StreamingAssetFile,
    ) -> streaming::error::Result<Self> {
        let root = payload.data.expect_object("animation metadata root")?;
        let channels_value = root.required_field("channels")?;
        let channel_values = channels_value.expect_array("animation channels")?;
        let declared_channel_count: usize = root
            .required_field("channel_count")?
            .expect_parse("animation channel_count")?;
        ensure_whatever!(
            declared_channel_count == channel_values.len(),
            "animation channel_count {} did not match channels length {}",
            declared_channel_count,
            channel_values.len()
        );

        let mut cursor = AnimationBlobCursor::new(&payload.blob_infos);
        let mut channels = Vec::with_capacity(channel_values.len());
        for channel_value in channel_values {
            channels.push(AnimationChannel::decode(
                channel_value,
                &mut cursor,
                package,
            )?);
        }
        cursor.ensure_exhausted()?;

        Ok(AnimationClip {
            name: root
                .required_field("name")?
                .expect_parse("animation name")?,
            duration: root
                .required_field("duration")?
                .expect_parse("animation duration")?,
            channels,
        })
    }
}

struct AnimationBlobCursor<'a> {
    by_kind: HashMap<StreamingAssetBlobKind, Vec<&'a StreamingAssetBlobInfo>>,
}

impl<'a> AnimationBlobCursor<'a> {
    fn new(blobs: &'a StreamingAssetBlobInfos) -> Self {
        let mut by_kind: HashMap<StreamingAssetBlobKind, Vec<&'a StreamingAssetBlobInfo>> =
            HashMap::new();
        for blob in &blobs.infos {
            by_kind.entry(blob.kind).or_default().push(blob);
        }
        Self { by_kind }
    }

    fn take(
        &mut self,
        kind: StreamingAssetBlobKind,
        expected_count: usize,
        label: &str,
    ) -> streaming::error::Result<Option<&'a StreamingAssetBlobInfo>> {
        if expected_count == 0 {
            return Ok(None);
        }

        let Some(blobs) = self.by_kind.get_mut(&kind) else {
            whatever!("missing {label} blob for {expected_count} entries");
        };

        if blobs.is_empty() {
            whatever!("missing {label} blob for {expected_count} entries");
        }

        Ok(Some(blobs.remove(0)))
    }

    fn ensure_exhausted(&self) -> streaming::error::Result<()> {
        for (kind, remaining) in &self.by_kind {
            if !remaining.is_empty() {
                whatever!(
                    "unused '{}' blob sections remained after animation decode ({})",
                    kind.name(),
                    remaining.len()
                );
            }
        }
        Ok(())
    }
}

impl AnimationChannel {
    fn decode(
        value: &JsonValue,
        cursor: &mut AnimationBlobCursor,
        package: &mut StreamingAssetFile,
    ) -> streaming::error::Result<AnimationChannel> {
        let channel = value.expect_object("animation channel")?;
        let keys = channel
            .required_field("keys")?
            .expect_object("animation keys")?;

        let t_times_count: usize = keys
            .required_field("t_times_count")?
            .expect_parse("animation t_times_count")?;
        let t_values_count: usize = keys
            .required_field("t_values_count")?
            .expect_parse("animation t_values_count")?;
        let r_times_count: usize = keys
            .required_field("r_times_count")?
            .expect_parse("animation r_times_count")?;
        let r_values_count: usize = keys
            .required_field("r_values_count")?
            .expect_parse("animation r_values_count")?;
        let s_times_count: usize = keys
            .required_field("s_times_count")?
            .expect_parse("animation s_times_count")?;
        let s_values_count: usize = keys
            .required_field("s_values_count")?
            .expect_parse("animation s_values_count")?;

        Ok(AnimationChannel {
            target_name: channel
                .required_field("target_name")?
                .expect_parse("animation target_name")?,
            keys: TransformKeys {
                t_times: cursor
                    .take(
                        StreamingAssetBlobKind::AnimationTranslationTimes,
                        t_times_count,
                        "animation translation times",
                    )?
                    .context(BlobNotFoundErr)?
                    .decode_from_io("animation translation times", t_times_count, package)?,
                t_values: cursor
                    .take(
                        StreamingAssetBlobKind::AnimationTranslationValues,
                        t_values_count,
                        "animation translation values",
                    )?
                    .context(BlobNotFoundErr)?
                    .decode_from_io("animation translation values", t_values_count, package)?,
                r_times: cursor
                    .take(
                        StreamingAssetBlobKind::AnimationRotationTimes,
                        r_times_count,
                        "animation rotation times",
                    )?
                    .context(BlobNotFoundErr)?
                    .decode_from_io("animation rotation times", r_times_count, package)?,
                r_values: cursor
                    .take(
                        StreamingAssetBlobKind::AnimationRotationValues,
                        r_values_count,
                        "animation rotation values",
                    )?
                    .context(BlobNotFoundErr)?
                    .decode_from_io("animation rotation values", r_values_count, package)?,
                s_times: cursor
                    .take(
                        StreamingAssetBlobKind::AnimationScaleTimes,
                        s_times_count,
                        "animation scale times",
                    )?
                    .context(BlobNotFoundErr)?
                    .decode_from_io("animation scale times", s_times_count, package)?,
                s_values: cursor
                    .take(
                        StreamingAssetBlobKind::AnimationScaleValues,
                        s_values_count,
                        "animation scale values",
                    )?
                    .context(BlobNotFoundErr)?
                    .decode_from_io("animation scale values", s_values_count, package)?,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_samples_edges_and_interpolates() {
        let keys = TransformKeys::keyed_translation(
            &[0.0, 1.0, 2.0],
            &[[0.0, 0.0, 0.0], [1.0, 2.0, 3.0], [2.0, 4.0, 6.0]],
        );

        assert_eq!(
            keys.sample_translation(-0.5).unwrap(),
            Vec3::new(-0.5, -1.0, -1.5)
        );
        assert_eq!(
            keys.sample_translation(2.5).unwrap(),
            Vec3::new(2.0, 4.0, 6.0)
        );

        let mid = keys.sample_translation(0.5).unwrap();
        assert!((mid - Vec3::new(0.5, 1.0, 1.5)).abs().max_element() < 1e-6);
    }

    #[test]
    fn scale_samples_single_entry_and_interpolates() {
        let single = TransformKeys::keyed_scale(&[0.0], &[[2.0, 2.0, 2.0]]);
        assert_eq!(single.sample_scale(5.0).unwrap(), Vec3::splat(2.0));

        let keys = TransformKeys::keyed_scale(&[0.0, 2.0], &[[1.0, 1.0, 1.0], [3.0, 5.0, 7.0]]);
        let mid = keys.sample_scale(1.0).unwrap();
        assert!((mid - Vec3::new(2.0, 3.0, 4.0)).abs().max_element() < f32::EPSILON);
    }

    #[test]
    fn rotation_slerps_between_quaternions() {
        let keys = TransformKeys::keyed_rotation(&[0.0, 1.0], &[0.0, std::f32::consts::FRAC_PI_2]);

        let start = keys.sample_rotation(-1.0).unwrap();
        let expected_start = keys.r_values[0].slerp(keys.r_values[1], -1.0).normalize();
        assert!(start.angle_between(expected_start) < f32::EPSILON);

        let end = keys.sample_rotation(2.0).unwrap();
        assert!((end.to_axis_angle().1 - std::f32::consts::FRAC_PI_2).abs() < f32::EPSILON);

        let mid = keys.sample_rotation(0.5).unwrap();
        assert!((mid.to_axis_angle().1 - std::f32::consts::FRAC_PI_4).abs() < f32::EPSILON);
    }

    #[test]
    fn empty_tracks_return_none() {
        let keys = TransformKeys::default();
        assert!(keys.sample_translation(0.0).is_none());
        assert!(keys.sample_scale(0.0).is_none());
        assert!(keys.sample_rotation(0.0).is_none());
    }
}
