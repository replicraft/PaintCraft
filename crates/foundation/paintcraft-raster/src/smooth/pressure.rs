//! PaintCraft pressure-diameter policy, version 1.
//!
//! Each actual sample supplies an optional normalized 16-bit pressure. Missing
//! pressure means full captured diameter; zero means no endpoint disk. Consecutive
//! samples sweep a round tip with linearly varying radius, unioned on the same 4×4 grid as
//! ordinary smooth strokes. Coverage is resolved once, so retracing does not blend
//! translucent joins repeatedly. This is a PaintCraft policy, not device emulation.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PressurePoint {
    /// Continuous document position in 1/256 pixel units, captured at ingestion.
    pub at: (i64, i64),
    pub pressure: Option<u16>,
}

/// Resolve one immutable actual-sample prefix. Predictions must never be supplied.
/// Work uses the same scan-row and point bounds as ordinary smooth paths.
pub fn pressure_path_checked(
    points: &[PressurePoint],
    size: u32,
    clip: &Rect,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<CoverageSpan>> {
    check()?;
    if points.is_empty() || points.len() > 65_536 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "invalid pressure sample count",
        ));
    }
    // Validate without allocating a second position vector.
    for chunk in points.chunks(256) {
        check()?;
        for point in chunk {
            validate(&[point.at], size, clip, 256)?;
        }
    }
    if clip.x0 >= clip.x1 || clip.y0 >= clip.y1 {
        return Ok(Vec::new());
    }
    let local_clip = Rect {
        x0: 0,
        y0: 0,
        x1: clip.width(),
        y1: clip.height(),
    };
    let local = |point: PressurePoint| {
        (
            (
                (point.at.0 - clip.x0 * 256) as f64 / 256.0,
                (point.at.1 - clip.y0 * 256) as f64 / 256.0,
            ),
            f64::from(size) * f64::from(point.pressure.unwrap_or(u16::MAX)) / 65535.0 / 2.0,
        )
    };
    let mut rows = BTreeMap::new();
    let mut charged = 0;
    let (mut previous, mut radius) = local(points[0]);
    if points.len() == 1 {
        scan_tapered_segment(
            previous,
            previous,
            (radius, radius),
            &local_clip,
            &mut rows,
            &mut charged,
            check,
        )?;
    }
    for point in &points[1..] {
        check()?;
        let (position, next_radius) = local(*point);
        scan_tapered_segment(
            previous,
            position,
            (radius, next_radius),
            &local_clip,
            &mut rows,
            &mut charged,
            check,
        )?;
        previous = position;
        radius = next_radius;
    }
    let mut spans = resolve_rows(&mut rows, check)?;
    for span in &mut spans {
        span.x0 += clip.x0;
        span.x1 += clip.x0;
        span.y += clip.y0;
    }
    Ok(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(x: i64, y: i64, pressure: Option<u16>) -> PressurePoint {
        PressurePoint {
            at: (x * 256, y * 256),
            pressure,
        }
    }
    fn coverage(spans: &[CoverageSpan], x: i64, y: i64) -> u8 {
        spans
            .iter()
            .find(|s| s.y == y && x >= s.x0 && x <= s.x1)
            .map_or(0, |s| s.samples)
    }
    #[test]
    fn missing_full_and_zero_pressure_have_explicit_geometry_and_stationary_growth() {
        let clip = Rect::new(-10, -10, 30, 30).unwrap();
        let positions = [(256, 512), (768, 1024), (2048, 256)];
        let points: Vec<_> = positions
            .iter()
            .map(|&at| PressurePoint { at, pressure: None })
            .collect();
        let ordinary = round_path_scaled_checked(&positions, 7, &clip, 256, &|| Ok(())).unwrap();
        assert_eq!(
            pressure_path_checked(&points, 7, &clip, &|| Ok(())).unwrap(),
            ordinary
        );
        let full: Vec<_> = points
            .iter()
            .map(|p| PressurePoint {
                pressure: Some(65535),
                ..*p
            })
            .collect();
        assert_eq!(
            pressure_path_checked(&full, 7, &clip, &|| Ok(())).unwrap(),
            ordinary
        );
        assert!(
            pressure_path_checked(&[point(2, 2, Some(0))], 7, &clip, &|| Ok(()))
                .unwrap()
                .is_empty()
        );
        let growth = [point(2, 2, Some(0)), point(2, 2, Some(65535))];
        assert_eq!(
            pressure_path_checked(&growth, 7, &clip, &|| Ok(())).unwrap(),
            pressure_path_checked(&growth[1..], 7, &clip, &|| Ok(())).unwrap()
        );
    }
    // Independent moving-disk oracle. Maximize r(t)^2-|p-c(t)|^2 on [0,1]
    // rather than using the production tangent/scanline geometry.
    fn inside(p: (f64, f64), a: (f64, f64), b: (f64, f64), radii: (f64, f64)) -> bool {
        if radii == (0.0, 0.0) {
            return false;
        }
        let d = (b.0 - a.0, b.1 - a.1);
        let q = (p.0 - a.0, p.1 - a.1);
        let dr = radii.1 - radii.0;
        let quadratic = dr * dr - d.0 * d.0 - d.1 * d.1;
        let linear = 2.0 * (radii.0 * dr + q.0 * d.0 + q.1 * d.1);
        let constant = radii.0 * radii.0 - q.0 * q.0 - q.1 * q.1;
        let at = |t: f64| (quadratic * t + linear) * t + constant;
        let mut maximum = at(0.0).max(at(1.0));
        if quadratic < 0.0 {
            let t = (-linear / (2.0 * quadratic)).clamp(0.0, 1.0);
            maximum = maximum.max(at(t));
        }
        maximum >= 0.0
    }
    #[test]
    fn tapered_union_matches_independent_subsample_oracle_and_retracing_is_idempotent() {
        let clip = Rect::new(-4, -4, 20, 20).unwrap();
        let path = [
            point(1, 2, Some(8192)),
            point(9, 7, Some(49151)),
            point(3, 10, Some(65535)),
        ];
        let spans = pressure_path_checked(&path, 7, &clip, &|| Ok(())).unwrap();
        for y in clip.y0..clip.y1 {
            for x in clip.x0..clip.x1 {
                let mut expected = 0;
                for sy in 0..4 {
                    for sx in 0..4 {
                        let p = (
                            x as f64 + (f64::from(sx) + 0.5) / 4.0,
                            y as f64 + (f64::from(sy) + 0.5) / 4.0,
                        );
                        if path.windows(2).any(|w| {
                            inside(
                                p,
                                (w[0].at.0 as f64 / 256.0, w[0].at.1 as f64 / 256.0),
                                (w[1].at.0 as f64 / 256.0, w[1].at.1 as f64 / 256.0),
                                (
                                    7.0 * f64::from(w[0].pressure.unwrap()) / 65535.0 / 2.0,
                                    7.0 * f64::from(w[1].pressure.unwrap()) / 65535.0 / 2.0,
                                ),
                            )
                        }) {
                            expected += 1;
                        }
                    }
                }
                assert_eq!(coverage(&spans, x, y), expected, "at ({x},{y})");
            }
        }
        let retraced = [path[0], path[1], path[2], path[1], path[0]];
        assert_eq!(
            pressure_path_checked(&retraced, 7, &clip, &|| Ok(())).unwrap(),
            spans
        );
    }
    #[test]
    fn sharp_ramps_reversals_and_containment_match_moving_disk_oracle() {
        let clip = Rect::new(-12, -12, 48, 36).unwrap();
        for (end, pressures) in [
            ((20, 0), (65535, 0)),
            ((20, 11), (65535, 4096)),
            ((4, 0), (65535, 0)),
            ((0, 0), (8192, 65535)),
        ] {
            let path = [
                point(0, 0, Some(pressures.0)),
                point(end.0, end.1, Some(pressures.1)),
            ];
            let spans = pressure_path_checked(&path, 20, &clip, &|| Ok(())).unwrap();
            assert_eq!(
                pressure_path_checked(&[path[1], path[0]], 20, &clip, &|| Ok(())).unwrap(),
                spans
            );
            for y in clip.y0..clip.y1 {
                for x in clip.x0..clip.x1 {
                    let mut expected = 0;
                    for sy in 0..4 {
                        for sx in 0..4 {
                            let p = (
                                x as f64 + (f64::from(sx) + 0.5) / 4.0,
                                y as f64 + (f64::from(sy) + 0.5) / 4.0,
                            );
                            expected += u8::from(inside(
                                p,
                                (0.0, 0.0),
                                (end.0 as f64, end.1 as f64),
                                (
                                    10.0 * f64::from(pressures.0) / 65535.0,
                                    10.0 * f64::from(pressures.1) / 65535.0,
                                ),
                            ));
                        }
                    }
                    assert_eq!(
                        coverage(&spans, x, y),
                        expected,
                        "end {end:?}, pressures {pressures:?}, at ({x},{y})"
                    );
                }
            }
            if end == (20, 0) {
                assert_eq!(coverage(&spans, 10, 5), 8);
            }
        }
        assert!(inside(
            (10.125, 5.625),
            (0.0, 0.0),
            (20.0, 0.0),
            (10.0, 0.0)
        ));
    }
    #[test]
    fn invalid_budget_and_cancel_refuse_without_truncating_a_sample_prefix() {
        use std::cell::Cell;
        let clip = Rect::new(0, 0, 32, 32).unwrap();
        let points = [point(1, 2, None), point(24, 24, Some(12345))];
        assert!(pressure_path_checked(&points, 0, &clip, &|| Ok(())).is_err());
        let count = Cell::new(0);
        pressure_path_checked(&points, 7, &clip, &|| {
            count.set(count.get() + 1);
            Ok(())
        })
        .unwrap();
        for stop in 1..=count.get() {
            let calls = Cell::new(0);
            let error = pressure_path_checked(&points, 7, &clip, &|| {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "stop pressure"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
        }
        let many = vec![point(4, 4, None); 65536];
        assert_eq!(
            pressure_path_checked(&many, 1024, &clip, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
    }
}
