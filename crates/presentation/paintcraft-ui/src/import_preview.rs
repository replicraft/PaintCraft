//! Display geometry for reviewing pixels without changing the imported image.
use egui::Vec2;

/// Fit the preview into at most 200 logical points per axis, including upscaling.
/// Exceptionally thin images retain a one-point visible axis where space permits.
pub fn fitted_size(source: Vec2, available: Vec2) -> Vec2 {
    if !source.is_finite() || source.min_elem() <= 0.0 {
        return Vec2::ZERO;
    }
    let available = available.min(Vec2::splat(200.0)).max(Vec2::ZERO);
    let scale = (available.x / source.x).min(available.y / source.y);
    (source * scale).max(Vec2::splat(1.0)).min(available)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_images_upscale_with_aspect_and_stay_inside_available_space() {
        for (source, available, expected) in [
            (Vec2::ONE, Vec2::splat(400.0), Vec2::splat(200.0)),
            (
                Vec2::new(3.0, 2.0),
                Vec2::splat(200.0),
                Vec2::new(200.0, 400.0 / 3.0),
            ),
            (
                Vec2::new(3.0, 2.0),
                Vec2::new(90.0, 30.0),
                Vec2::new(45.0, 30.0),
            ),
        ] {
            let size = fitted_size(source, available);
            assert!((size - expected).length() < 0.001);
            assert!(size.x <= available.x && size.y <= available.y);
            assert!((size.x / size.y - source.x / source.y).abs() < 0.001);
        }
    }

    #[test]
    fn thin_axes_remain_visible_without_exceeding_small_or_empty_areas() {
        for source in [Vec2::new(65537.0, 1.0), Vec2::new(1.0, 65537.0)] {
            let size = fitted_size(source, Vec2::splat(200.0));
            assert_eq!(size.min_elem(), 1.0);
            assert_eq!(size.max_elem(), 200.0);
            assert_eq!(fitted_size(source, Vec2::ZERO), Vec2::ZERO);
            assert_eq!(fitted_size(source, Vec2::splat(0.5)), Vec2::splat(0.5));
        }
    }
}
