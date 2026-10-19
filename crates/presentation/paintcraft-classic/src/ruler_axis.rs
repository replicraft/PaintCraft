//! Pixel tick placement is bounded by the visible interval, not document size.
#[derive(Debug, PartialEq)]
pub(super) struct Tick {
    pub position: f32,
    pub value: i64,
    pub major: bool,
}

pub(super) fn ticks(origin: f32, scale: f32, extent: u32, visible: (f32, f32)) -> Vec<Tick> {
    if !origin.is_finite()
        || !scale.is_finite()
        || scale <= 0.0
        || !visible.0.is_finite()
        || !visible.1.is_finite()
        || visible.0 > visible.1
    {
        return Vec::new();
    }
    let scale = f64::from(scale);
    let target = (75.0 / scale).max(1.0);
    let base = 10.0_f64.powf(target.log10().floor());
    let major = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|n| n * base)
        .find(|n| *n >= target)
        .unwrap_or(base * 10.0)
        .min(i64::MAX as f64) as i64;
    let minor = (major / 5).max(1);
    let first = ((f64::from(visible.0) - f64::from(origin)) / scale).max(0.0);
    let last = ((f64::from(visible.1) - f64::from(origin)) / scale).min(f64::from(extent));
    if first > last {
        return Vec::new();
    }
    let start = (first / minor as f64).ceil() as i64;
    let end = (last / minor as f64).floor() as i64;
    (start..=end)
        .take(8192)
        .map(|index| {
            let value = index * minor;
            Tick {
                value,
                major: value % major == 0,
                position: (f64::from(origin) + value as f64 * scale) as f32,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_share_canvas_pixel_mapping_at_fractional_zoom_and_display_scales() {
        for ppp in [1.0, 1.5, 2.0] {
            for zoom in [0.125, 0.73, 1.0, 4.0, 64.0] {
                let scale = zoom / ppp;
                let view = paintcraft_ui::view::View::with_scale((-37.25, 0.0), zoom, ppp);
                let marks = ticks(-37.25, scale, 1048576, (0.0, 800.0));
                assert!(!marks.is_empty());
                assert!(
                    marks
                        .iter()
                        .all(
                            |tick| (tick.position - view.to_screen((tick.value, 0)).0).abs()
                                < 0.001
                        )
                );
                assert!(
                    marks
                        .iter()
                        .all(|tick| (0.0..=800.0).contains(&tick.position))
                );
                assert!(marks.iter().any(|tick| tick.major));
            }
        }
    }

    #[test]
    fn scrolling_changes_visible_ticks_without_rebasing_document_units() {
        let marks = ticks(-10000.0, 1.0, 1048576, (0.0, 80.0));
        assert!(marks.len() <= 6);
        assert_eq!(
            marks[0],
            Tick {
                position: 0.0,
                value: 10000,
                major: true
            }
        );
        assert!(
            marks
                .iter()
                .all(|tick| tick.value >= 10000 && tick.value <= 10080)
        );
        assert!(ticks(100.0, 1.0, 50, (0.0, 90.0)).is_empty());
        assert!(ticks(0.0, 1.0, 50, (100.0, 200.0)).is_empty());
    }

    #[test]
    fn deep_zoom_ticks_stay_on_whole_pixels_and_bad_inputs_are_empty() {
        let marks = ticks(0.0, 64.0, 1000, (0.0, 256.0));
        assert_eq!(
            marks.iter().map(|tick| tick.value).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]
        );
        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(ticks(0.0, scale, 1000, (0.0, 800.0)).is_empty());
        }
    }
}
