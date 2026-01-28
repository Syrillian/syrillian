use crate::math::{EulerRot, Quat, Vec3};

pub trait QuaternionEuler<T> {
    fn euler_vector_deg(&self) -> Vec3;
    fn euler_vector(&self) -> Vec3;
    fn from_euler_angles_deg(roll: T, pitch: T, yaw: T) -> Quat;
}

impl QuaternionEuler<f32> for Quat {
    fn euler_vector_deg(&self) -> Vec3 {
        let angles = self.euler_vector();
        Vec3::new(
            angles.x.to_degrees(),
            angles.y.to_degrees(),
            angles.z.to_degrees(),
        )
    }

    fn euler_vector(&self) -> Vec3 {
        self.to_euler(EulerRot::XYZ).into()
    }

    fn from_euler_angles_deg(roll: f32, pitch: f32, yaw: f32) -> Quat {
        Quat::from_euler(
            EulerRot::XYZ,
            roll.to_radians(),
            pitch.to_radians(),
            yaw.to_radians(),
        )
    }
}

pub trait FloatMathExt {
    fn lerp(self, other: Self, t: f32) -> Self;
}

impl FloatMathExt for f32 {
    fn lerp(self, other: Self, t: f32) -> Self {
        self * (1.0 - t) + other * t
    }
}

#[allow(non_snake_case)]
pub fn light_range(E: f32, a0: f32, a1: f32, a2: f32, T: f32) -> Option<f32> {
    if T <= 0.0 || E <= 0.0 {
        return Some(0.0);
    }

    // constant-only attenuation
    if a2 == 0.0 && a1 == 0.0 {
        let val_at_0 = E / a0.max(1e-12);
        return if val_at_0 <= T { Some(0.0) } else { None };
    }

    let at0 = E / a0.max(1e-12);
    if at0 <= T {
        return Some(0.0);
    }

    if a2 == 0.0 {
        if a1 == 0.0 {
            return None;
        }
        let d = (E / T - a0) / a1;
        return if d.is_finite() && d > 0.0 {
            Some(d)
        } else {
            Some(0.0)
        };
    }

    let c = a0 - E / T;
    let disc = a1 * a1 - 4.0 * a2 * c;
    if disc < 0.0 {
        return Some(0.0);
    }
    let sqrt_d = disc.sqrt();
    let dpos = (-a1 + sqrt_d) / (2.0 * a2);
    if dpos.is_finite() && dpos > 0.0 {
        Some(dpos)
    } else {
        Some(0.0)
    }
}
