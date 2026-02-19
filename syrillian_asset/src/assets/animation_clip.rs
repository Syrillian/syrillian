use crate::store::{H, HandleName, StoreType};
use glamx::{EulerRot, Quat, Vec3};
use std::cmp::Ordering;

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
