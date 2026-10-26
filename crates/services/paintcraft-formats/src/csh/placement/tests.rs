use super::*;
use std::cell::Cell;
fn unit(value: f64) -> i32 {
    (value * (1u32 << 24) as f64).round() as i32
}
fn knot(incoming: (f64, f64), anchor: (f64, f64), outgoing: (f64, f64)) -> Record {
    Record::Knot {
        open: false,
        linked: false,
        points: [
            unit(incoming.0),
            unit(incoming.1),
            unit(anchor.0),
            unit(anchor.1),
            unit(outgoing.0),
            unit(outgoing.1),
        ],
    }
}
fn rectangle() -> Shape {
    let mut records = vec![
        Record::FillRule(0),
        Record::InitialFill(false),
        Record::Subpath {
            open: false,
            knots: 4,
            operation: Some(Operation::Combine),
            fill_rule: PathFillRule::EvenOdd,
        },
    ];
    for point in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
        records.push(knot(point, point, point));
    }
    Shape {
        name: "Rectangle".into(),
        id: "rectangle".into(),
        bounds: [19, 23, 27, 31],
        records,
    }
}
fn library(shape: Shape) -> Arc<Library> {
    Arc::new(Library::new(vec![shape], &|| Ok(())).unwrap())
}
fn render(shape: Shape, width: u32, height: u32) -> Rasterized {
    rasterize(
        library(shape),
        0,
        width,
        height,
        Policy::CubicFillV1,
        &SharedBudget::new(64 << 20),
        &|| Ok(()),
    )
    .unwrap()
}
fn alpha(raster: &Rasterized) -> Vec<u8> {
    raster
        .rgba(Rgba([1, 2, 3, 255]), &SharedBudget::new(64 << 20), &|| {
            Ok(())
        })
        .unwrap()
        .rgba()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[3])
        .collect()
}
#[test]
fn rectangle_pixels_are_independent_of_source_origin_and_close_edge_orientation() {
    let shape = rectangle();
    let source = library(shape.clone());
    let original = source.shapes().to_vec();
    let raster = rasterize(
        source.clone(),
        0,
        7,
        5,
        Policy::CubicFillV1,
        &SharedBudget::new(64 << 20),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(alpha(&raster), vec![255; 35]);
    assert_eq!(raster.source().shapes(), original);
    assert!(Arc::ptr_eq(&source, raster.source()));
    assert_eq!(raster.shape_index(), 0);
    let mut reversed = shape;
    reversed.records[3..].reverse();
    assert_eq!(alpha(&render(reversed, 7, 5)), vec![255; 35]);
    assert_eq!(
        raster
            .rgba(
                Rgba([9, 18, 27, 128]),
                &SharedBudget::new(64 << 20),
                &|| Ok(())
            )
            .unwrap()
            .rgba(),
        [9, 18, 27, 128].repeat(35)
    );
}
fn lens() -> Shape {
    Shape {
        name: "Cubic lens".into(),
        id: "lens".into(),
        bounds: [100, 200, 132, 232],
        records: vec![
            Record::FillRule(0),
            Record::InitialFill(false),
            Record::Subpath {
                open: false,
                knots: 2,
                operation: None,
                fill_rule: PathFillRule::EvenOdd,
            },
            knot((0.0, 1.0), (0.0, 0.5), (0.0, 0.0)),
            knot((1.0, 0.0), (1.0, 0.5), (1.0, 1.0)),
        ],
    }
}
#[test]
fn genuine_cubic_controls_and_closing_curve_match_an_independent_analytic_interior() {
    let mask = alpha(&render(lens(), 32, 32));
    let mut partial = 0;
    for y in 0..32 {
        for x in 0..32 {
            // x(t)=3t²-2t³, y_upper(t)=.5(1-t)^3+.5t^3;
            // the closing segment is its reflection about y=.5.
            let px = (x as f64 + 0.5) / 32.0;
            let py = (y as f64 + 0.5) / 32.0;
            let (mut low, mut high) = (0.0, 1.0);
            for _ in 0..60 {
                let t = (low + high) / 2.0;
                if 3.0 * t * t - 2.0 * t * t * t < px {
                    low = t;
                } else {
                    high = t;
                }
            }
            let t = (low + high) / 2.0;
            let upper = 0.5 * ((1.0 - t).powi(3) + t.powi(3));
            let lower = 1.0 - upper;
            let distance = (py - upper).abs().min((py - lower).abs());
            if distance > 2.0 / 32.0 {
                assert_eq!(
                    mask[y * 32 + x],
                    if py > upper && py < lower { 255 } else { 0 },
                    "({x},{y})"
                );
            }
            if mask[y * 32 + x] > 0 && mask[y * 32 + x] < 255 {
                partial += 1;
            }
        }
    }
    assert!(partial > 0);
    assert_eq!(
        mask[24 * 32 + 16],
        255,
        "closing cubic must not become a straight close edge"
    );
    assert_eq!(
        mask[7 * 32 + 16],
        255,
        "top cubic must not become an anchor polygon"
    );
    assert_eq!(mask[16], 0);
    let mut reversed = lens();
    let old: Vec<_> = reversed.records.drain(3..).collect();
    reversed
        .records
        .extend(old.into_iter().rev().map(|record| match record {
            Record::Knot {
                open,
                linked,
                points: [a, b, c, d, e, f],
            } => Record::Knot {
                open,
                linked,
                points: [e, f, c, d, a, b],
            },
            _ => unreachable!(),
        }));
    let opposite = alpha(&render(reversed, 32, 32));
    assert!(mask.iter().zip(opposite).all(|(a, b)| a.abs_diff(b) <= 1));
}
#[test]
fn declared_evenodd_and_nonzero_fill_are_distinct_instead_of_unioned() {
    let mut shape = rectangle();
    let copy = shape.records[3..].to_vec();
    shape.records.extend(copy);
    if let Record::Subpath { knots, .. } = &mut shape.records[2] {
        *knots = 8;
    }
    assert!(alpha(&render(shape.clone(), 8, 8)).iter().all(|v| *v == 0));
    if let Record::Subpath { fill_rule, .. } = &mut shape.records[2] {
        *fill_rule = PathFillRule::NonZero;
    }
    assert_eq!(alpha(&render(shape, 8, 8)), vec![255; 64]);
}
#[test]
fn unsupported_geometry_and_size_fail_before_producing_any_pixels() {
    let original = rectangle();
    let mut candidates = Vec::new();
    for operation in [
        Operation::Exclude,
        Operation::Subtract,
        Operation::Intersect,
    ] {
        let mut s = original.clone();
        if let Record::Subpath { operation: op, .. } = &mut s.records[2] {
            *op = Some(operation);
        }
        candidates.push(s);
    }
    let mut s = original.clone();
    s.records[1] = Record::InitialFill(true);
    candidates.push(s);
    let mut s = original.clone();
    s.records.insert(1, Record::Clipboard([0; 5]));
    candidates.push(s);
    let mut s = original.clone();
    s.records.extend(original.records[2..].to_vec());
    candidates.push(s);
    let mut s = original.clone();
    for record in &mut s.records {
        match record {
            Record::Subpath { open, .. } | Record::Knot { open, .. } => *open = true,
            _ => {}
        }
    }
    candidates.push(s);
    let mut s = original.clone();
    if let Record::Knot { points, .. } = &mut s.records[3] {
        points[0] = i32::MAX;
    }
    candidates.push(s);
    for candidate in candidates {
        let source = library(candidate);
        let before = source.shapes().to_vec();
        assert!(!assess(&source.shapes()[0], &|| Ok(())).unwrap().qualified());
        assert_eq!(
            rasterize(
                source.clone(),
                0,
                8,
                8,
                Policy::CubicFillV1,
                &SharedBudget::new(64 << 20),
                &|| Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
        assert_eq!(source.shapes(), before);
    }
    for (w, h) in [(0, 8), (8, 0), (SIDE_MAX + 1, 1), (4096, 4096)] {
        assert_eq!(
            rasterize(
                library(original.clone()),
                0,
                w,
                h,
                Policy::CubicFillV1,
                &SharedBudget::new(64 << 20),
                &|| Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::ResourceLimit
        );
    }
}
#[test]
fn every_cancellation_checkpoint_and_preview_failure_keep_exact_source_unchanged() {
    let source = library(lens());
    let before = source.shapes().to_vec();
    let calls = Cell::new(0);
    let rendered = rasterize(
        source.clone(),
        0,
        32,
        32,
        Policy::CubicFillV1,
        &SharedBudget::new(64 << 20),
        &|| {
            calls.set(calls.get() + 1);
            Ok(())
        },
    )
    .unwrap();
    for stop in 0..calls.get() {
        let count = Cell::new(0);
        let result = rasterize(
            source.clone(),
            0,
            32,
            32,
            Policy::CubicFillV1,
            &SharedBudget::new(64 << 20),
            &|| {
                let n = count.get();
                count.set(n + 1);
                if n == stop {
                    Err(Error::new(ErrorCode::OperationCancelled, "test"))
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.unwrap_err().code, ErrorCode::OperationCancelled);
        assert_eq!(source.shapes(), before);
    }
    assert!(
        rendered
            .rgba(Rgba([1, 2, 3, 255]), &SharedBudget::new(64 << 20), &|| Err(
                Error::new(ErrorCode::OperationCancelled, "test")
            ))
            .is_err()
    );
    assert_eq!(source.shapes(), before);
}
#[test]
fn float_renderer_precision_is_explicit_and_does_not_rewrite_signed_824_source() {
    let a = (2 * UNIT - 1) as i32;
    let b = (2 * UNIT) as i32;
    assert_ne!(f64::from(a) / UNIT as f64, f64::from(b) / UNIT as f64);
    assert_eq!(map_coordinate(a, 4096), map_coordinate(b, 4096));
    let mut shape = rectangle();
    if let Record::Knot { points, .. } = &mut shape.records[3] {
        points[0] = a;
        points[4] = b;
    }
    let source = library(shape);
    let exact = source.shapes().to_vec();
    let _ = rasterize(
        source.clone(),
        0,
        8,
        8,
        Policy::CubicFillV1,
        &SharedBudget::new(64 << 20),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(source.shapes(), exact);
}
#[test]
fn signed_placement_clips_exactly_and_rejects_coordinate_overflow() {
    let raster = render(rectangle(), 4, 3);
    let clip = paintcraft_core::types::Rect::new(0, 0, 8, 8).unwrap();
    let spans = raster
        .translated_spans((-2, 1), clip, &SharedBudget::new(64 << 20), &|| Ok(()))
        .unwrap();
    assert_eq!(
        spans.spans(),
        (1..4)
            .map(|y| MaskSpan {
                y,
                x0: 0,
                x1: 1,
                coverage: 255
            })
            .collect::<Vec<_>>()
    );
    assert!(
        raster
            .translated_spans((8, 8), clip, &SharedBudget::new(64 << 20), &|| Ok(()))
            .unwrap()
            .spans()
            .is_empty()
    );
    for clip in [
        paintcraft_core::types::Rect {
            x0: 0,
            y0: 0,
            x1: i64::MIN,
            y1: 1,
        },
        paintcraft_core::types::Rect {
            x0: 0,
            y0: 0,
            x1: i64::MAX,
            y1: 1,
        },
    ] {
        assert_eq!(
            raster
                .translated_spans((0, 0), clip, &SharedBudget::new(64 << 20), &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
    for at in [
        (i64::MAX, 0),
        (0, i64::MIN),
        (paintcraft_core::types::policy::COORD_ABS_MAX, 0),
    ] {
        assert_eq!(
            raster
                .translated_spans(at, clip, &SharedBudget::new(64 << 20), &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::InputSchema
        );
    }
}
#[test]
fn independent_wire_fixture_keeps_geometry_and_refuses_unsupported_presets() {
    let bytes = include_bytes!("../../../tests/csh-fixtures/placement.csh");
    let source = Arc::new(decode(bytes, &|| Ok(())).unwrap());
    assert_eq!(source.shapes().len(), 4);
    assert_eq!(source.shapes()[0], rectangle());
    assert_eq!(source.shapes()[1], lens());
    assert_eq!(
        alpha(
            &rasterize(
                source.clone(),
                0,
                8,
                8,
                Policy::CubicFillV1,
                &SharedBudget::new(64 << 20),
                &|| Ok(())
            )
            .unwrap()
        ),
        vec![255; 64]
    );
    assert_eq!(
        alpha(
            &rasterize(
                source.clone(),
                1,
                32,
                32,
                Policy::CubicFillV1,
                &SharedBudget::new(64 << 20),
                &|| Ok(())
            )
            .unwrap()
        ),
        alpha(&render(lens(), 32, 32))
    );
    for index in [2, 3] {
        assert_eq!(
            rasterize(
                source.clone(),
                index,
                32,
                32,
                Policy::CubicFillV1,
                &SharedBudget::new(64 << 20),
                &|| Ok(())
            )
            .unwrap_err()
            .code,
            ErrorCode::CapabilityUnsupported
        );
    }
    assert_eq!(
        decode(&encode(&source, &|| Ok(())).unwrap(), &|| Ok(())).unwrap(),
        *source
    );
}
#[test]
fn quota_denial_precedes_pixel_allocation_and_owned_buffers_release_on_last_drop() {
    let source = library(lens());
    let before = source.shapes().to_vec();
    let denied = SharedBudget::new(1);
    let error = rasterize(
        source.clone(),
        0,
        32,
        32,
        Policy::CubicFillV1,
        &denied,
        &|| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(error.allocation_denial().is_some());
    assert_eq!(denied.usage().reserved, 0);
    assert_eq!(source.shapes(), before);
    let budget = SharedBudget::new(1 << 20);
    let rendered = rasterize(
        source.clone(),
        0,
        32,
        32,
        Policy::CubicFillV1,
        &budget,
        &|| Ok(()),
    )
    .unwrap();
    let retained = rendered.coverage.declared_bytes();
    assert!(retained > 0);
    assert_eq!(budget.usage().reserved, retained);
    let clone = rendered.clone();
    drop(rendered);
    assert_eq!(budget.usage().reserved, retained);
    let preview = clone
        .rgba(Rgba([1, 2, 3, 128]), &budget, &|| Ok(()))
        .unwrap();
    assert_eq!(preview.declared_bytes(), 4096);
    let copy = preview.declare_copy(&budget).unwrap();
    assert_eq!(budget.usage().reserved, retained + 8192);
    let translated = clone
        .translated_spans(
            (0, 0),
            paintcraft_core::types::Rect::new(0, 0, 32, 32).unwrap(),
            &budget,
            &|| Ok(()),
        )
        .unwrap();
    let translated_clone = translated.clone();
    drop(translated);
    assert!(budget.usage().reserved > retained + 8192);
    drop(translated_clone);
    assert_eq!(budget.usage().reserved, retained + 8192);
    drop(clone);
    assert_eq!(budget.usage().reserved, 8192);
    drop(preview);
    assert_eq!(budget.usage().reserved, 4096);
    drop(copy);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn cancellation_and_failed_preview_admission_release_all_temporary_leases() {
    let source = library(lens());
    let budget = SharedBudget::new(1 << 20);
    let total = Cell::new(0);
    drop(
        rasterize(
            source.clone(),
            0,
            32,
            32,
            Policy::CubicFillV1,
            &budget,
            &|| {
                total.set(total.get() + 1);
                Ok(())
            },
        )
        .unwrap(),
    );
    assert_eq!(budget.usage().reserved, 0);
    for stop in 0..total.get() {
        let calls = Cell::new(0);
        assert!(
            rasterize(
                source.clone(),
                0,
                32,
                32,
                Policy::CubicFillV1,
                &budget,
                &|| {
                    let count = calls.get();
                    calls.set(count + 1);
                    if count == stop {
                        Err(Error::new(ErrorCode::OperationCancelled, "test"))
                    } else {
                        Ok(())
                    }
                }
            )
            .is_err()
        );
        assert_eq!(budget.usage().reserved, 0, "checkpoint {stop}");
    }
    let rendered = rasterize(source, 0, 32, 32, Policy::CubicFillV1, &budget, &|| Ok(())).unwrap();
    let before = budget.usage().reserved;
    let small = SharedBudget::new(4095);
    assert_eq!(
        rendered
            .rgba(Rgba([0, 0, 0, 255]), &small, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(small.usage().reserved, 0);
    assert!(
        rendered
            .rgba(Rgba([0, 0, 0, 255]), &budget, &|| Err(Error::new(
                ErrorCode::OperationCancelled,
                "test"
            )))
            .is_err()
    );
    assert_eq!(budget.usage().reserved, before);
    let preview = rendered
        .rgba(Rgba([0, 0, 0, 255]), &budget, &|| Ok(()))
        .unwrap();
    assert!(preview.declare_copy(&small).is_err());
    assert_eq!(small.usage().reserved, 0);
    drop(preview);
    drop(rendered);
    assert_eq!(budget.usage().reserved, 0);
}
