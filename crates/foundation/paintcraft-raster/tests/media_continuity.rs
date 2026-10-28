use paintcraft_core::types::{Rect, brush::BrushMedia, resources::SharedBudget};
use paintcraft_raster::media::dynamics::{self, Attributes, Point};
use std::collections::BTreeMap;

fn mask(points: &[Point], media: BrushMedia, size: u32, clip: Rect) -> BTreeMap<(i64, i64), u8> {
    let mut output = BTreeMap::new();
    dynamics::visit_checked(
        points,
        size,
        media,
        clip,
        &SharedBudget::new(8 << 20),
        &|| Ok(()),
        &mut |span| {
            for x in span.x0..=span.x1 {
                output.insert((x, span.y), span.coverage);
            }
            Ok(())
        },
    )
    .unwrap();
    output
}
fn point(x: f64, y: f64, width: u16) -> Point {
    Point {
        at: ((x * 256.0).round() as i64, (y * 256.0).round() as i64),
        attributes: Attributes {
            width,
            ..Default::default()
        },
    }
}

#[test]
fn one_percent_width_tips_remain_connected_without_periodic_horizontal_scallops() {
    let clip = Rect::new(0, 0, 128, 96).unwrap();
    let mut failures = 0;
    for medium in [
        BrushMedia::Plain,
        BrushMedia::CalligraphyBrush,
        BrushMedia::CalligraphyPen,
        BrushMedia::Marker,
    ] {
        for slope in [0.0, 0.5] {
            let points = [
                point(16.5, 32.5, 655),
                point(112.5, 32.5 + 96.0 * slope, 655),
            ];
            let pixels = mask(&points, medium, 128, clip);
            let mut missing = 0;
            let mut coverage = Vec::new();
            for x in 18..=110 {
                let y = (32.5 + (x as f64 - 16.0) * slope).floor() as i64;
                let value = (y - 1..=y + 1)
                    .map(|y| pixels.get(&(x, y)).copied().unwrap_or(0))
                    .max()
                    .unwrap();
                missing += usize::from(value == 0);
                coverage.push(value);
            }
            let variation = coverage.iter().max().unwrap() - coverage.iter().min().unwrap();
            eprintln!(
                "{medium:?} size128 width655/65535 slope{slope}: missing_columns={missing}, central_variation={variation}"
            );
            if missing != 0 || (slope == 0.0 && variation > 16) {
                failures += 1;
            }
            let reverse = mask(&[points[1], points[0]], medium, 128, clip);
            assert_eq!(pixels, reverse, "{medium:?} reverse slope{slope}");
        }
    }
    assert_eq!(
        failures, 0,
        "low-width paths must have connected support and stable straight interiors"
    );
}

#[test]
fn pressure_ramps_and_low_width_sampling_do_not_restart_at_clip_edges() {
    let outer = Rect::new(0, 0, 160, 128).unwrap();
    let inner = Rect::new(30, 35, 65, 40).unwrap();
    for medium in [
        BrushMedia::Plain,
        BrushMedia::CalligraphyBrush,
        BrushMedia::CalligraphyPen,
        BrushMedia::Marker,
    ] {
        for widths in [[655, 655], [655, 65535], [65535, 0]] {
            let points = [point(10.5, 24.5, widths[0]), point(140.5, 95.5, widths[1])];
            let full = mask(&points, medium, 128, outer);
            let cropped = full
                .into_iter()
                .filter(|((x, y), _)| inner.contains(*x, *y))
                .collect();
            assert_eq!(
                mask(&points, medium, 128, inner),
                cropped,
                "{medium:?} {widths:?}"
            );
        }
    }
}

#[test]
fn resampled_dabs_have_an_independent_allowance_above_the_raw_sample_count() {
    let clip = Rect::new(0, 0, 4352, 128).unwrap();
    // Effective width~1.28 / narrow calligraphy nib uses a1/16px step:
    //4160px requires66,560 segment intervals, beyond the old65,536 cap.
    let points = [point(16.5, 64.5, 655), point(4176.5, 64.5, 655)];
    let output = mask(&points, BrushMedia::CalligraphyPen, 128, clip);
    assert!((18..4175).all(|x| output.get(&(x, 64)).is_some_and(|&a| a != 0)));
}
