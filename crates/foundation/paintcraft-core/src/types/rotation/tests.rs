use super::*;
use crate::types::json::{self, Value};
fn number(value: &Value) -> i64 {
    value.as_i64().unwrap()
}
fn rect(value: &Value) -> Rect {
    let v = value.as_array().unwrap();
    Rect {
        x0: number(&v[0]),
        y0: number(&v[1]),
        x1: number(&v[2]),
        y1: number(&v[3]),
    }
}
#[test]
fn independent_fraction_bounds_and_every_inverse_pixel_center() {
    let oracle = json::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/rotation-q30-fixtures/geometry.json"
    )))
    .unwrap();
    let cases = oracle.get("cases").unwrap().as_array().unwrap();
    assert_eq!(cases.len(), 27);
    for case in cases {
        let source = rect(case.get("source").unwrap());
        let pair = case.get("coefficients").unwrap().as_array().unwrap();
        let g = Rotation::new(source, (number(&pair[0]) as i32, number(&pair[1]) as i32)).unwrap();
        let bounds = rect(case.get("bounds").unwrap());
        assert_eq!(g.bounds(source).unwrap(), bounds);
        assert_eq!(
            g.bounds(rect(case.get("other_extent").unwrap())).unwrap(),
            rect(case.get("other_bounds").unwrap())
        );
        let samples = case.get("source_indices").unwrap().as_array().unwrap();
        let mut n = 0;
        for y in bounds.y0..bounds.y1 {
            for x in bounds.x0..bounds.x1 {
                let p = samples[n].as_array().unwrap();
                assert_eq!(
                    g.source_pixel(x, y).unwrap(),
                    (number(&p[0]), number(&p[1]))
                );
                n += 1;
            }
        }
        assert_eq!(n, samples.len());
    }
}
#[test]
fn exact_legacy_cardinals_match_odd_even_anchor_and_pixel_orientation() {
    for (w, h) in [(2, 3), (3, 2), (1, 4), (5, 5)] {
        let source = Rect::new(-7, 9, w, h).unwrap();
        for (pair, legacy) in [
            ((0, COEFFICIENT_ONE), OrthogonalTransform::RotateRight),
            ((-COEFFICIENT_ONE, 0), OrthogonalTransform::Rotate180),
            ((0, -COEFFICIENT_ONE), OrthogonalTransform::RotateLeft),
        ] {
            let g = Rotation::new(source, pair).unwrap();
            assert_eq!(g.exact_orientation(), Some(legacy));
            let (dw, dh) = legacy.dimensions(w as u32, h as u32);
            let b = g.bounds(source).unwrap();
            assert_eq!((b.x0, b.y0), (source.x0, source.y0));
            assert_eq!((b.width(), b.height()), (i64::from(dw), i64::from(dh)));
            for y in 0..i64::from(dh) {
                for x in 0..i64::from(dw) {
                    let expected = match legacy {
                        OrthogonalTransform::RotateRight => (y, h - 1 - x),
                        OrthogonalTransform::RotateLeft => (w - 1 - y, x),
                        OrthogonalTransform::Rotate180 => (w - 1 - x, h - 1 - y),
                        _ => unreachable!(),
                    };
                    assert_eq!(
                        g.source_pixel(source.x0 + x, source.y0 + y).unwrap(),
                        (source.x0 + expected.0, source.y0 + expected.1)
                    );
                }
            }
        }
    }
}
#[test]
fn identity_and_near_cardinal_coefficients_are_not_silently_snapped() {
    let r = Rect::new(0, 0, 2, 3).unwrap();
    let g = Rotation::new(r, (COEFFICIENT_ONE, 0)).unwrap();
    assert!(g.is_identity());
    assert_eq!(g.bounds(r).unwrap(), r);
    for y in -1..4 {
        for x in -1..3 {
            assert_eq!(g.source_pixel(x, y).unwrap(), (x, y));
        }
    }
    let near = Rotation::new(r, (1, COEFFICIENT_ONE)).unwrap();
    assert_eq!(near.exact_orientation(), None);
    assert!(!near.is_identity());
    assert_ne!(near.bounds(r).unwrap().width(), 3);
}
#[test]
fn malformed_coefficients_and_original_geometry_limits_refuse_without_saturating() {
    let r = Rect::new(0, 0, 2, 3).unwrap();
    for pair in [
        (0, 0),
        (COEFFICIENT_ONE, COEFFICIENT_ONE),
        (i32::MIN, 0),
        (COEFFICIENT_ONE - 3, 0),
    ] {
        assert_eq!(
            Rotation::new(r, pair).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
    let bad = Rect {
        x0: i64::MIN,
        y0: 0,
        x1: i64::MAX,
        y1: 1,
    };
    assert_eq!(
        Rotation::new(bad, (COEFFICIENT_ONE, 0)).unwrap_err().code,
        ErrorCode::ResourceLimit
    );
    let edge = Rotation::new(
        Rect::new(COORD_ABS_MAX - 2, 0, 2, 3).unwrap(),
        (COEFFICIENT_ONE, 0),
    )
    .unwrap();
    assert!(edge.source_pixel(COORD_ABS_MAX + 1, 0).is_err());
    assert!(
        Rotation::new(
            Rect::new(0, 0, i64::from(CANVAS_SIDE_MAX), i64::from(CANVAS_SIDE_MAX)).unwrap(),
            (759250125, 759250125)
        )
        .is_err()
    );
}

#[test]
fn one_pinned_degree_conversion_exact_cardinals_and_invalid_angles() {
    let q = COEFFICIENT_ONE;
    for (angle, pair) in [
        (0.0, (q, 0)),
        (90.0, (0, q)),
        (180.0, (-q, 0)),
        (270.0, (0, -q)),
        (-90.0, (0, -q)),
        (-180.0, (-q, 0)),
        (-270.0, (0, q)),
        (360.0, (q, 0)),
        (-360.0, (q, 0)),
    ] {
        assert_eq!(super::coefficients_from_degrees(angle).unwrap(), pair);
    }
    for bad in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        360.001,
        -360.001,
    ] {
        assert_eq!(
            super::coefficients_from_degrees(bad).unwrap_err().code,
            ErrorCode::InputSchema
        );
    }
    for angle in [-359.999, -135.0, -45.0, 0.001, 33.3, 45.0, 89.999, 359.999] {
        let p = super::coefficients_from_degrees(angle).unwrap();
        let n = i128::from(p.0) * i128::from(p.0) + i128::from(p.1) * i128::from(p.1);
        assert!((n - (1i128 << 60)).abs() <= 1i128 << 31);
    }
}
