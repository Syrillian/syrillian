use crate::core::GameObjectId;
use crate::math::{Quat, Vec3};
use std::cmp::Ordering;
use std::collections::HashMap;

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

#[derive(Debug, Clone)]
pub struct Channel {
    pub target_name: String,
    pub keys: TransformKeys,
}

#[derive(Debug, Clone)]
pub struct AnimationClip {
    pub name: String,
    /// Duration in seconds
    pub duration: f32,
    pub channels: Vec<Channel>,
}

#[derive(Debug, Default, Clone)]
pub struct ClipIndex {
    pub by_name: HashMap<String, usize>,
}

impl ClipIndex {
    pub fn new(clip: &AnimationClip) -> Self {
        let mut by_name = HashMap::new();
        for (i, ch) in clip.channels.iter().enumerate() {
            by_name.insert(ch.target_name.clone(), i);
        }
        Self { by_name }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Binding {
    Transform(GameObjectId),
    Bone { skel: GameObjectId, idx: usize },
}

#[derive(Debug, Clone)]
pub struct ChannelBinding {
    /// Index into clip.channels
    pub ch_index: usize,
    pub target: Binding,
}

#[derive(Debug, Clone)]
pub struct Playback {
    pub clip_index: usize,
    pub time: f32,
    pub speed: f32,
    pub weight: f32,
    pub looping: bool,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            clip_index: 0,
            time: 0.0,
            speed: 1.0,
            weight: 1.0,
            looping: true,
        }
    }
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

fn lerp_vec3(a: &Vec3, b: &Vec3, alpha: f32) -> Vec3 {
    a * (1.0 - alpha) + b * alpha
}

pub fn sample_translation(keys: &TransformKeys, t: f32) -> Option<Vec3> {
    let n = keys.t_times.len();
    if n == 0 {
        return None;
    }
    if n == 1 {
        return Some(keys.t_values[0]);
    }

    let i = find_key(&keys.t_times, t);
    if i == n - 1 {
        return Some(keys.t_values[i]);
    }
    let t0 = keys.t_times[i];
    let t1 = keys.t_times[i + 1];
    let a = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
    Some(lerp_vec3(&keys.t_values[i], &keys.t_values[i + 1], a))
}

pub fn sample_scale(keys: &TransformKeys, t: f32) -> Option<Vec3> {
    let n = keys.s_times.len();
    if n == 0 {
        return None;
    }
    if n == 1 {
        return Some(keys.s_values[0]);
    }

    let i = find_key(&keys.s_times, t);
    if i == n - 1 {
        return Some(keys.s_values[i]);
    }
    let t0 = keys.s_times[i];
    let t1 = keys.s_times[i + 1];
    let a = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
    Some(lerp_vec3(&keys.s_values[i], &keys.s_values[i + 1], a))
}

pub fn sample_rotation(keys: &TransformKeys, t: f32) -> Option<Quat> {
    let n = keys.r_times.len();
    if n == 0 {
        return None;
    }
    if n == 1 {
        return Some(keys.r_values[0]);
    }

    let i = find_key(&keys.r_times, t);
    if i == n - 1 {
        return Some(keys.r_values[i]);
    }
    let t0 = keys.r_times[i];
    let t1 = keys.r_times[i + 1];
    let a = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
    Some(keys.r_values[i].slerp(keys.r_values[i + 1], a).normalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::EulerRot;

    fn keyed_translation(times: &[f32], values: &[[f32; 3]]) -> TransformKeys {
        TransformKeys {
            t_times: times.to_vec(),
            t_values: values.iter().map(|v| Vec3::new(v[0], v[1], v[2])).collect(),
            ..Default::default()
        }
    }

    fn keyed_scale(times: &[f32], values: &[[f32; 3]]) -> TransformKeys {
        TransformKeys {
            s_times: times.to_vec(),
            s_values: values.iter().map(|v| Vec3::new(v[0], v[1], v[2])).collect(),
            ..Default::default()
        }
    }

    fn keyed_rotation(times: &[f32], angles: &[f32]) -> TransformKeys {
        TransformKeys {
            r_times: times.to_vec(),
            r_values: angles
                .iter()
                .map(|a| Quat::from_euler(EulerRot::XYZ, 0.0, 0.0, *a))
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn translation_samples_edges_and_interpolates() {
        let keys = keyed_translation(
            &[0.0, 1.0, 2.0],
            &[[0.0, 0.0, 0.0], [1.0, 2.0, 3.0], [2.0, 4.0, 6.0]],
        );

        assert_eq!(
            sample_translation(&keys, -0.5).unwrap(),
            Vec3::new(-0.5, -1.0, -1.5)
        );
        assert_eq!(
            sample_translation(&keys, 2.5).unwrap(),
            Vec3::new(2.0, 4.0, 6.0)
        );

        let mid = sample_translation(&keys, 0.5).unwrap();
        assert!((mid - Vec3::new(0.5, 1.0, 1.5)).abs().max_element() < 1e-6);
    }

    #[test]
    fn scale_samples_single_entry_and_interpolates() {
        let single = keyed_scale(&[0.0], &[[2.0, 2.0, 2.0]]);
        assert_eq!(sample_scale(&single, 5.0).unwrap(), Vec3::splat(2.0));

        let keys = keyed_scale(&[0.0, 2.0], &[[1.0, 1.0, 1.0], [3.0, 5.0, 7.0]]);
        let mid = sample_scale(&keys, 1.0).unwrap();
        assert!((mid - Vec3::new(2.0, 3.0, 4.0)).abs().max_element() < f32::EPSILON);
    }

    #[test]
    fn rotation_slerps_between_quaternions() {
        let keys = keyed_rotation(&[0.0, 1.0], &[0.0, std::f32::consts::FRAC_PI_2]);

        let start = sample_rotation(&keys, -1.0).unwrap();
        let expected_start = keys.r_values[0].slerp(keys.r_values[1], -1.0).normalize();
        assert!(start.angle_between(expected_start) < f32::EPSILON);

        let end = sample_rotation(&keys, 2.0).unwrap();
        assert!((end.to_axis_angle().1 - std::f32::consts::FRAC_PI_2).abs() < f32::EPSILON);

        let mid = sample_rotation(&keys, 0.5).unwrap();
        assert!((mid.to_axis_angle().1 - std::f32::consts::FRAC_PI_4).abs() < f32::EPSILON);
    }

    #[test]
    fn empty_tracks_return_none() {
        let keys = TransformKeys::default();
        assert!(sample_translation(&keys, 0.0).is_none());
        assert!(sample_scale(&keys, 0.0).is_none());
        assert!(sample_rotation(&keys, 0.0).is_none());
    }
}
