use glamx::Vec3;

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Vec3 {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let hp = h / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (r1, g1, b1) = match hp.floor() as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        5 => (c, 0.0, x),
        _ => (0.0, 0.0, 0.0),
    };
    Vec3::new(r1, g1, b1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_wraps_and_maps_primary_colors() {
        // Red at full saturation/value
        let red = hsv_to_rgb(0.0, 1.0, 1.0);
        assert!((red - Vec3::new(1.0, 0.0, 0.0)).abs().max_element() < f32::EPSILON);

        // Green via hue wrap
        let green = hsv_to_rgb(120.0, 1.0, 1.0);
        assert!((green - Vec3::new(0.0, 1.0, 0.0)).abs().max_element() < f32::EPSILON);

        // Blue with negative hue wrapping correctly
        let blue = hsv_to_rgb(-120.0, 1.0, 1.0);
        assert!((blue - Vec3::new(0.0, 0.0, 1.0)).abs().max_element() < f32::EPSILON);
    }

    #[test]
    fn hsv_handles_partial_saturation() {
        let color = hsv_to_rgb(60.0, 0.5, 0.5);
        // Expect non-zero red/green, zero blue
        assert!(color.x > 0.0 && color.y > 0.0);
        assert!(color.z.abs() < 1e-6);
        assert!(
            (color.x - color.y).abs() < 1e-6,
            "yellow components should match"
        );
    }
}
