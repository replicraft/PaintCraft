use paintcraft_core::types::{Error, ErrorCode, Rect, Rgba};
use paintcraft_raster::{BlendMode, PaintPlan};
use std::cell::Cell;

fn plan(color: Rgba, samples: u8, reverse: bool) -> PaintPlan {
    let mut plan = PaintPlan::default();
    let areas = [
        Rect::new(0, 0, 2, 2).unwrap(),
        Rect::new(1, 1, 2, 2).unwrap(),
    ];
    for index in if reverse { [1, 0] } else { [0, 1] } {
        plan.coverage_rect(areas[index], color, samples).unwrap();
    }
    plan
}

#[test]
fn exact_equality_distinguishes_coverage_color_mode_order_and_cow_changes() {
    let color = Rgba([21, 43, 65, 128]);
    let original = plan(color, 7, false);
    let independent = plan(color, 7, false);
    assert!(
        original
            .same_writes_checked(&independent, &|| Ok(()))
            .unwrap()
    );
    for different in [
        plan(color, 8, false),
        plan(Rgba([22, 43, 65, 128]), 7, false),
        plan(color, 7, true),
    ] {
        assert!(
            !original
                .same_writes_checked(&different, &|| Ok(()))
                .unwrap()
        );
    }
    let mut changed = original.clone();
    changed
        .rect(Rect::new(1, 1, 2, 2).unwrap(), color, BlendMode::SourceOver)
        .unwrap();
    assert!(!original.same_writes_checked(&changed, &|| Ok(())).unwrap());
    assert!(
        original
            .same_writes_checked(&independent, &|| Ok(()))
            .unwrap()
    );
    let mut replace = PaintPlan::default();
    replace
        .rect(Rect::new(0, 0, 1, 1).unwrap(), color, BlendMode::Replace)
        .unwrap();
    let mut over = PaintPlan::default();
    over.rect(Rect::new(0, 0, 1, 1).unwrap(), color, BlendMode::SourceOver)
        .unwrap();
    assert!(!replace.same_writes_checked(&over, &|| Ok(())).unwrap());
}

#[test]
fn equal_long_plans_check_cancellation_without_modification_or_unbounded_walks() {
    let make = || {
        let mut plan = PaintPlan::default();
        for x in 0..4097 {
            plan.rect(
                Rect::new(x * 2, 0, 1, 1).unwrap(),
                Rgba([1; 4]),
                BlendMode::Replace,
            )
            .unwrap();
        }
        plan
    };
    let a = make();
    let b = make();
    let calls = Cell::new(0);
    assert!(
        a.same_writes_checked(&b, &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap()
    );
    assert!(calls.get() >= 6);
    for cancel_at in 1..=calls.get() {
        let count = Cell::new(0);
        let error = a
            .same_writes_checked(&b, &|| {
                count.set(count.get() + 1);
                if count.get() == cancel_at {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel descriptor comparison",
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
    assert!(a.same_writes_checked(&b, &|| Ok(())).unwrap());
}
