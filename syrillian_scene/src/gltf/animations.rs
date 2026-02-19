use crate::GltfScene;
use gltf::animation::util::ReadOutputs;
use syrillian::math::{Quat, Vec3};
use syrillian_asset::{AnimationChannel, AnimationClip, TransformKeys};

impl GltfScene {
    /// Builds animation clips from the glTF scene
    pub fn decode_animations(&self) -> Vec<AnimationClip> {
        let mut clips = Vec::<AnimationClip>::new();

        for anim in self.doc.animations() {
            let clip = self.build_animation_clip(anim);
            if !clip.channels.is_empty() {
                clips.push(clip);
            }
        }

        clips
    }

    /// Converts a glTF animation into an engine animation clip
    pub fn build_animation_clip(&self, anim: gltf::Animation) -> AnimationClip {
        let name = anim.name().unwrap_or("Animation").to_string();
        let (channels, duration) = self.collect_animation_channels(anim);

        AnimationClip {
            name,
            duration,
            channels,
        }
    }

    /// Collects all channels of a glTF animation
    pub fn collect_animation_channels(
        &self,
        anim: gltf::Animation,
    ) -> (Vec<AnimationChannel>, f32) {
        let mut channels = Vec::new();
        let mut max_time = 0.0f32;

        for ch in anim.channels() {
            if let Some((channel, duration)) = self.read_channel(ch) {
                channels.push(channel);
                max_time = max_time.max(duration);
            }
        }

        (channels, max_time)
    }

    /// Reads a single animation channel and converts it into an engine animation channel
    pub fn read_channel(
        &self,
        channel: gltf::animation::Channel,
    ) -> Option<(AnimationChannel, f32)> {
        let node = channel.target().node();
        let target_name = node
            .name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("node{}", node.index()));

        let reader = channel.reader(|b| Some(&self.buffers[b.index()].0));
        let times: Vec<f32> = reader.read_inputs()?.collect();
        let duration = times.last().copied().unwrap_or(0.0);
        let outputs = reader.read_outputs()?;
        let keys = build_transform_keys(outputs, &times);

        Some((AnimationChannel { target_name, keys }, duration))
    }
}

/// Builds transform keyframes from glTF animation outputs
fn build_transform_keys(outputs: ReadOutputs, times: &[f32]) -> TransformKeys {
    match outputs {
        ReadOutputs::Translations(values) => {
            let translations: Vec<Vec3> = values.into_iter().map(Vec3::from).collect();
            TransformKeys {
                t_times: times.to_vec(),
                t_values: translations,
                ..TransformKeys::default()
            }
        }
        ReadOutputs::Rotations(values) => {
            let rotations: Vec<Quat> = values.into_f32().map(Quat::from_array).collect();
            TransformKeys {
                r_times: times.to_vec(),
                r_values: rotations,
                ..TransformKeys::default()
            }
        }
        ReadOutputs::Scales(values) => {
            let scales: Vec<Vec3> = values.into_iter().map(Vec3::from).collect();
            TransformKeys {
                s_times: times.to_vec(),
                s_values: scales,
                ..TransformKeys::default()
            }
        }
        _ => TransformKeys::default(),
    }
}
