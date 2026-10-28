use paintcraft_core::types::{Error, ErrorCode, Rect, brush::BrushMedia, resources::SharedBudget};
use paintcraft_raster::media::{
    self,
    dynamics::{self, Attributes, Point},
};
use std::{cell::Cell, collections::BTreeMap};

fn pixels(points: &[Point], media: BrushMedia, clip: Rect) -> BTreeMap<(i64, i64), u8> {
    let budget = SharedBudget::new(8 << 20);
    let mut pixels = BTreeMap::new();
    dynamics::visit_checked(points, 16, media, clip, &budget, &|| Ok(()), &mut |span| {
        for x in span.x0..=span.x1 {
            pixels.insert((x, span.y), span.coverage);
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.usage().reserved, 0);
    pixels
}
fn point(x: i64, y: i64, attributes: Attributes) -> Point {
    Point {
        at: (x * 256, y * 256),
        attributes,
    }
}

#[test]
fn cancellation_keeps_priority_and_invisible_dabs_cannot_evade_the_work_bound() {
    let clip = Rect::new(0, 0, 32, 32).unwrap();
    assert_eq!(
        media::coverage(&[], 0, 0, BrushMedia::Plain, clip, &|| {
            Err(Error::new(ErrorCode::OperationCancelled, "cancel first"))
        })
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    for attributes in [
        Attributes {
            width: 0,
            ..Default::default()
        },
        Attributes {
            opacity: 0,
            ..Default::default()
        },
    ] {
        let path = (0..33000)
            .map(|i| point(if i % 2 == 0 { 8 } else { 24 }, 16, attributes))
            .collect::<Vec<_>>();
        let budget = SharedBudget::new(1 << 20);
        let delivered = Cell::new(0);
        assert_eq!(
            dynamics::visit_checked(
                &path,
                1,
                BrushMedia::Marker,
                clip,
                &budget,
                &|| Ok(()),
                &mut |_| {
                    delivered.set(delivered.get() + 1);
                    Ok(())
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(delivered.get(), 0);
        assert_eq!(budget.usage().reserved, 0);
    }
}

#[test]
fn full_attributes_keep_every_existing_textured_media_pixel() {
    let clip = Rect::new(0, 0, 96, 64).unwrap();
    let points = [
        (8 * 256 + 128, 9 * 256 + 128),
        (80 * 256 + 128, 48 * 256 + 128),
        (19 * 256 + 128, 45 * 256 + 128),
    ];
    let dynamic = points.map(|at| Point {
        at,
        attributes: Attributes::default(),
    });
    for medium in BrushMedia::ALL.into_iter().skip(1) {
        let expected: BTreeMap<_, _> = media::coverage(&points, 16, 256, medium, clip, &|| Ok(()))
            .unwrap()
            .into_iter()
            .flat_map(|span| (span.x0..=span.x1).map(move |x| ((x, span.y), span.coverage)))
            .collect();
        assert_eq!(pixels(&dynamic, medium, clip), expected, "{medium:?}");
    }
}

#[test]
fn pressure_opacity_is_a_maximum_stroke_cap_and_width_zero_is_empty() {
    let clip = Rect::new(0, 0, 64, 64).unwrap();
    for medium in BrushMedia::ALL {
        let full = point(32, 32, Attributes::default());
        let half = point(
            32,
            32,
            Attributes {
                opacity: 32768,
                ..Default::default()
            },
        );
        let expected = pixels(&[full], medium, clip)
            .into_iter()
            .map(|(p, alpha)| (p, ((u32::from(alpha) * 32768 + 32767) / 65535) as u8))
            .filter(|(_, alpha)| *alpha != 0)
            .collect();
        assert_eq!(pixels(&[half], medium, clip), expected, "{medium:?}");
        assert_eq!(
            pixels(&[half, half, half], medium, clip),
            expected,
            "no flow accumulation: {medium:?}"
        );
        assert!(
            pixels(
                &[point(
                    32,
                    32,
                    Attributes {
                        width: 0,
                        ..Default::default()
                    }
                )],
                medium,
                clip
            )
            .is_empty()
        );
        let narrow = pixels(
            &[point(
                32,
                32,
                Attributes {
                    width: 16384,
                    ..Default::default()
                },
            )],
            medium,
            clip,
        );
        assert_ne!(narrow, pixels(&[full], medium, clip), "{medium:?}");
    }
}

#[test]
fn actual_nibs_rotate_while_paper_texture_and_unsupported_media_are_not_reinterpreted() {
    let clip = Rect::new(0, 0, 64, 64).unwrap();
    for medium in [
        BrushMedia::CalligraphyBrush,
        BrushMedia::CalligraphyPen,
        BrushMedia::Marker,
    ] {
        let fixed = pixels(&[point(32, 32, Attributes::default())], medium, clip);
        let rotated = pixels(
            &[point(
                32,
                32,
                Attributes {
                    rotation_q8: 90 * 256,
                    ..Default::default()
                },
            )],
            medium,
            clip,
        );
        let expected: BTreeMap<_, _> = fixed
            .iter()
            .map(|(&(x, y), &alpha)| ((63 - y, x), alpha))
            .collect();
        assert_eq!(rotated, expected, "{medium:?}");
        assert_ne!(fixed, rotated, "{medium:?}");
    }
    assert!(
        dynamics::visit_checked(
            &[point(
                32,
                32,
                Attributes {
                    rotation_q8: 256,
                    ..Default::default()
                }
            )],
            16,
            BrushMedia::Oil,
            clip,
            &SharedBudget::new(8 << 20),
            &|| Ok(()),
            &mut |_| Ok(())
        )
        .is_err()
    );
}

#[test]
fn clipping_cancellation_and_admission_do_not_deliver_a_partial_mask() {
    let clip = Rect::new(0, 0, 96, 64).unwrap();
    let inner = Rect::new(20, 16, 40, 20).unwrap();
    let points = [
        point(
            8,
            9,
            Attributes {
                width: 5000,
                opacity: 12000,
                ..Default::default()
            },
        ),
        point(80, 48, Attributes::default()),
    ];
    for medium in BrushMedia::ALL {
        let full = pixels(&points, medium, clip);
        let expected = full
            .into_iter()
            .filter(|((x, y), _)| inner.contains(*x, *y))
            .collect();
        assert_eq!(pixels(&points, medium, inner), expected, "{medium:?}");
    }
    let emitted = Cell::new(0);
    let tiny = SharedBudget::new(1);
    assert_eq!(
        dynamics::visit_checked(
            &points,
            16,
            BrushMedia::Marker,
            clip,
            &tiny,
            &|| Ok(()),
            &mut |_| {
                emitted.set(emitted.get() + 1);
                Ok(())
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(emitted.get(), 0);
    assert_eq!(tiny.usage().reserved, 0);
    let budget = SharedBudget::new(8 << 20);
    let checks = Cell::new(0);
    assert_eq!(
        dynamics::visit_checked(
            &points,
            16,
            BrushMedia::Marker,
            clip,
            &budget,
            &|| {
                checks.set(checks.get() + 1);
                if checks.get() > 8 {
                    Err(Error::new(ErrorCode::OperationCancelled, "stop"))
                } else {
                    Ok(())
                }
            },
            &mut |_| {
                emitted.set(emitted.get() + 1);
                Ok(())
            }
        )
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    assert_eq!(emitted.get(), 0);
    assert_eq!(budget.usage().reserved, 0);
}
