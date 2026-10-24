use paintcraft_core::types::{
    Error, ErrorCode, Rect, Rgba, resources::SharedBudget, text::MaskSpan,
};
use paintcraft_raster::{
    Admission, BlendMode, PaintPlan, RasterLayer,
    selection::{BinaryMask, MaskLimits},
};
use std::cell::Cell;
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn field(budget: &SharedBudget) -> BinaryMask {
    let spans: Vec<_> = (-2..6)
        .flat_map(|y| {
            (-4..9)
                .filter(move |x| (x + y) % 3 == 0)
                .map(move |x| MaskSpan {
                    y,
                    x0: x,
                    x1: x,
                    coverage: 255,
                })
        })
        .collect();
    BinaryMask::from_spans(
        rect(-4, -2, 13, 8),
        false,
        &spans,
        budget,
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap()
}
fn plan() -> PaintPlan {
    let mut plan = PaintPlan::default();
    plan.rect(
        rect(-2, -1, 13, 9),
        Rgba([17, 88, 156, 128]),
        BlendMode::SourceOver,
    )
    .unwrap();
    plan.coverage_rect(rect(0, 0, 8, 6), Rgba([220, 40, 90, 0]), 7)
        .unwrap();
    plan.rect(
        rect(2, 1, 4, 3),
        Rgba([31, 63, 127, 73]),
        BlendMode::SourceOver,
    )
    .unwrap();
    plan
}
fn rendered(plan: &PaintPlan) -> RasterLayer {
    let mut layer = RasterLayer::new(8, 6, Rgba([37, 19, 83, 0]));
    plan.apply_checked(&mut layer, &mut Admission::new(65_536), &|| Ok(()))
        .unwrap();
    layer
}
#[test]
fn masks_preserve_order_coverage_hidden_rgb_and_translated_finite_views() {
    let budget = SharedBudget::new(1 << 20);
    let base = field(&budget);
    let source = plan();
    let reference = rendered(&source);
    for origin in [(0, 0), (-3, -2), (4, 3)] {
        for inverted in [false, true] {
            let view = if inverted {
                base.inverted()
            } else {
                base.clone()
            };
            let mask = view.clipped(rect(-2, -1, 8, 5)).unwrap().unwrap();
            let filtered = source
                .masked_checked(&mask, origin, &mut 100_000, &|| Ok(()))
                .unwrap();
            let result = rendered(&filtered);
            for y in 0..6 {
                for x in 0..8 {
                    let (dx, dy) = (x + origin.0, y + origin.1);
                    let inside = (-2..6).contains(&dx) && (-1..4).contains(&dy);
                    let selected = inside && (((dx + dy) % 3 == 0) ^ inverted);
                    assert_eq!(
                        result.get(x, y),
                        if selected {
                            reference.get(x, y)
                        } else {
                            Rgba([37, 19, 83, 0])
                        },
                        "{origin:?} {inverted} {x},{y}"
                    );
                }
            }
        }
    }
}
#[test]
fn uniform_fields_share_contained_plans_and_clip_without_row_scans() {
    let source = plan();
    let domain = rect(-5, -5, 30, 30);
    let full = BinaryMask::uniform(domain, true).unwrap();
    let mut work = 3;
    let filtered = source
        .masked_checked(&full, (1, 2), &mut work, &|| Ok(()))
        .unwrap();
    assert_eq!(work, 0);
    assert!(filtered.same_writes_checked(&source, &|| Ok(())).unwrap());
    assert_eq!(rendered(&filtered).get(3, 2), rendered(&source).get(3, 2));
    let empty = source
        .masked_checked(&full.inverted(), (0, 0), &mut 0, &|| Ok(()))
        .unwrap();
    assert!(empty.is_empty());
    let narrow = BinaryMask::uniform(rect(2, 1, 2, 2), true).unwrap();
    let clipped = rendered(
        &source
            .masked_checked(&narrow, (0, 0), &mut 10, &|| Ok(()))
            .unwrap(),
    );
    let reference = rendered(&source);
    for y in 0..6 {
        for x in 0..8 {
            assert_eq!(
                clipped.get(x, y),
                if (2..4).contains(&x) && (1..3).contains(&y) {
                    reference.get(x, y)
                } else {
                    Rgba([37, 19, 83, 0])
                }
            );
        }
    }
}
#[test]
fn cancellation_and_work_denials_release_output_and_preserve_input() {
    let selection = field(&SharedBudget::new(1 << 20));
    let metadata = SharedBudget::new(1 << 20);
    let mut source = PaintPlan::with_metadata_budget(1 << 20, metadata.clone());
    source
        .rect(
            rect(-4, -2, 13, 8),
            Rgba([1, 2, 3, 128]),
            BlendMode::SourceOver,
        )
        .unwrap();
    let before = metadata.usage().reserved;
    let calls = Cell::new(0);
    drop(
        source
            .masked_checked(&selection, (0, 0), &mut 100_000, &|| {
                calls.set(calls.get() + 1);
                Ok(())
            })
            .unwrap(),
    );
    assert_eq!(metadata.usage().reserved, before);
    for at in 1..=calls.get() {
        let n = Cell::new(0);
        let error = source
            .masked_checked(&selection, (0, 0), &mut 100_000, &|| {
                n.set(n.get() + 1);
                if n.get() == at {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel filtering",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(metadata.usage().reserved, before);
    }
    let mut work = 2;
    assert_eq!(
        source
            .masked_checked(&selection, (0, 0), &mut work, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(metadata.usage().reserved, before);
    assert_eq!(rendered(&source).get(0, 0), Rgba([1, 2, 3, 128]));
}
#[test]
fn output_denial_uses_original_metadata_realm_and_releases_private_prefix() {
    let budget = SharedBudget::new(1024);
    let mut source = PaintPlan::with_metadata_budget(1024, budget.clone());
    source
        .rect(
            rect(-4, -2, 13, 8),
            Rgba([1, 2, 3, 255]),
            BlendMode::Replace,
        )
        .unwrap();
    let before = budget.usage().reserved;
    let mask = field(&SharedBudget::new(1 << 20));
    let error = source
        .masked_checked(&mask, (0, 0), &mut 100_000, &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert_eq!(budget.usage().reserved, before);
    let full = BinaryMask::uniform(rect(-4, -2, 13, 8), true).unwrap();
    // Sharing the exact contained plan consumes no additional descriptor capacity.
    let shared = source
        .masked_checked(&full, (0, 0), &mut 1, &|| Ok(()))
        .unwrap();
    assert_eq!(budget.usage().reserved, before);
    drop(shared);
}
#[test]
fn work_is_shared_across_calls_and_coordinate_overflow_fails_closed() {
    let source = plan();
    let full = BinaryMask::uniform(rect(-5, -5, 30, 30), true).unwrap();
    let mut work = 5;
    source
        .masked_checked(&full, (0, 0), &mut work, &|| Ok(()))
        .unwrap();
    assert_eq!(work, 2);
    assert_eq!(
        source
            .masked_checked(&full, (0, 0), &mut work, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        source
            .masked_checked(&full, (i64::MAX, 0), &mut 10, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
}
