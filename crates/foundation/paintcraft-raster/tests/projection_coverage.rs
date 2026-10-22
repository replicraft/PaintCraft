use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba};
use paintcraft_raster::{BlendMode, PaintPlan};
use std::cell::Cell;

fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn source(x: i64, y: i64) -> Rgba {
    Rgba([x as u8, y as u8, 37, 0])
}

#[test]
fn outside_write_envelope_keeps_one_source_read_errors_and_bounded_cancellation() {
    let mut plan = PaintPlan::default();
    for _ in 0..4096 {
        plan.rect(rect(100, 1, 1, 1), Rgba([17; 4]), BlendMode::SourceOver)
            .unwrap();
    }
    let projected = plan
        .into_projection_checked(rect(0, 0, 256, 4), &|| Ok(()))
        .unwrap();
    let source_calls = Cell::new(0);
    let checks = Cell::new(0);
    assert_eq!(
        projected
            .read_checked(
                0,
                0,
                |x, y| {
                    source_calls.set(source_calls.get() + 1);
                    Ok(source(x, y))
                },
                &|| {
                    checks.set(checks.get() + 1);
                    Ok(())
                }
            )
            .unwrap(),
        source(0, 0)
    );
    assert_eq!(source_calls.get(), 1);
    assert!(
        checks.get() <= 3,
        "noncovering reads must not scan the long write list"
    );
    for at in 1..=checks.get() {
        let count = Cell::new(0);
        let error = projected
            .read_checked(0, 0, |x, y| Ok(source(x, y)), &|| {
                count.set(count.get() + 1);
                if count.get() == at {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel envelope read",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
    let error = projected
        .read_checked(
            0,
            0,
            |_, _| {
                Err(Error::new(
                    ErrorCode::ConflictStaleBase,
                    "source error outside writes",
                ))
            },
            &|| Ok(()),
        )
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ConflictStaleBase);
    // Outside the layer, the existing transparent/no-source policy still wins.
    assert_eq!(
        projected
            .read_checked(256, 0, |_, _| panic!("outside layer"), &|| Ok(()))
            .unwrap(),
        Rgba::TRANSPARENT
    );
}

#[test]
fn appended_clipped_coverage_and_holes_match_an_independent_pixel_oracle() {
    let bounds = rect(0, 0, 259, 4);
    let a = Rgba([241, 91, 7, 128]);
    let b = Rgba([51, 197, 123, 0]);
    let c = Rgba([211, 29, 161, 255]);
    let mut plan = PaintPlan::default();
    plan.rect(rect(-4, -2, 6, 4), a, BlendMode::SourceOver)
        .unwrap();
    let projected = plan.into_projection_checked(bounds, &|| Ok(())).unwrap();
    let mut incoming = PaintPlan::default();
    incoming.coverage_rect(rect(257, 1, 20, 4), b, 7).unwrap();
    incoming
        .rect(rect(258, 3, 1, 1), c, BlendMode::Replace)
        .unwrap();
    // These out-of-bounds writes cannot enlarge visibility or overflow a union.
    incoming
        .rect(rect(i64::MAX - 2, 0, 1, 1), c, BlendMode::Replace)
        .unwrap();
    let projected = projected.append_checked(incoming, &|| Ok(())).unwrap();
    for y in 0..4 {
        for x in 0..259 {
            let mut expected = source(x, y);
            if x < 2 && y < 2 {
                expected = a.source_over(expected);
            }
            if x >= 257 && y >= 1 {
                expected = Rgba(std::array::from_fn(|i| {
                    ((u16::from(b.0[i]) * 7 + u16::from(expected.0[i]) * 9 + 8) / 16) as u8
                }));
            }
            if x == 258 && y == 3 {
                expected = c;
            }
            assert_eq!(
                projected
                    .read_checked(x, y, |x, y| Ok(source(x, y)), &|| Ok(()))
                    .unwrap(),
                expected,
                "{x},{y}"
            );
        }
    }
}
