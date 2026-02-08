use syrillian::math::Quat;
use syrillian_utils::math::{FloatMathExt, QuaternionEuler, light_range};

#[test]
fn quaternion_euler_round_trip_degrees() {
    let q = Quat::from_euler_angles_deg(0.1, -0.2, 0.3);
    let deg = q.euler_vector_deg();

    let rebuilt = Quat::from_euler_angles_deg(deg.x, deg.y, deg.z);
    assert!(q.angle_between(rebuilt) < 1e-5);
}

#[test]
fn float_math_lerp_interpolates_between_values() {
    let start = 10.0_f32;
    let end = 20.0_f32;

    assert!((start.lerp(end, 0.0) - start).abs() < 1e-6);
    assert!((start.lerp(end, 0.5) - 15.0).abs() < 1e-6);
    assert!((start.lerp(end, 1.0) - end).abs() < 1e-6);
}

#[test]
fn light_range_handles_constant_and_quadratic_terms() {
    // constant-only attenuation within threshold
    assert_eq!(light_range(50.0, 10.0, 0.0, 0.0, 100.0), Some(0.0));

    // quadratic attenuation that should return a positive finite distance
    let range = light_range(100.0, 1.0, 0.7, 0.2, 1.0).expect("range should exist");
    assert!(range >= 0.0);
}
