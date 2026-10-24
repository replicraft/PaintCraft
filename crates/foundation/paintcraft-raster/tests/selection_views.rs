use paintcraft_core::types::{Error, ErrorCode, Rect, resources::SharedBudget, text::MaskSpan};
use paintcraft_raster::{
    Selection,
    selection::{BinaryMask, MaskLimits},
};
use std::cell::Cell;
fn mask(b: &SharedBudget) -> BinaryMask {
    BinaryMask::from_spans(
        Rect::new(-4, -2, 12, 8).unwrap(),
        false,
        &[MaskSpan {
            y: 0,
            x0: -3,
            x1: 4,
            coverage: 255,
        }],
        b,
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap()
}
#[test]
fn exact_field_comparison_distinguishes_views_and_bounds_work() {
    let budget = SharedBudget::new(1 << 20);
    let a = mask(&budget);
    let b = mask(&budget);
    assert_ne!(a, b);
    assert!(a.same_coverage_checked(&b, 96, &|| Ok(())).unwrap());
    assert!(
        !a.same_coverage_checked(&a.inverted(), 96, &|| Ok(()))
            .unwrap()
    );
    assert_eq!(
        a.same_coverage_checked(&b, 95, &|| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    let calls = Cell::new(0);
    assert_eq!(
        a.same_coverage_checked(&b, 96, &|| {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel compare"))
            } else {
                Ok(())
            }
        })
        .unwrap_err()
        .code,
        ErrorCode::OperationCancelled
    );
    let rect = Rect::new(-1, 2, 9, 7).unwrap();
    assert!(
        Selection::Rect(rect)
            .same_field_checked(
                &Selection::Mask(BinaryMask::uniform(rect, true).unwrap()),
                0,
                &|| Ok(())
            )
            .unwrap()
    );
}
#[test]
fn reframing_zeros_hidden_samples_and_is_failure_atomic() {
    let budget = SharedBudget::new(1 << 20);
    let a = mask(&budget);
    let clipped = a
        .clipped(Rect::new(-1, -1, 3, 3).unwrap())
        .unwrap()
        .unwrap();
    let before = budget.usage().reserved;
    let domain = Rect::new(-6, -4, 17, 13).unwrap();
    let calls = Cell::new(0);
    let expanded = clipped
        .reframed(domain, &budget, MaskLimits::default(), &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    for y in domain.y0..domain.y1 {
        for x in domain.x0..domain.x1 {
            assert_eq!(expanded.coverage(x, y), clipped.coverage(x, y));
        }
    }
    drop(expanded);
    assert_eq!(budget.usage().reserved, before);
    for at in 1..=calls.get() {
        let n = Cell::new(0);
        let error = clipped
            .reframed(domain, &budget, MaskLimits::default(), &|| {
                n.set(n.get() + 1);
                if n.get() == at {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel reframe"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(budget.usage().reserved, before);
    }
    let denied = SharedBudget::new(0);
    assert!(
        clipped
            .reframed(domain, &denied, MaskLimits::default(), &|| Ok(()))
            .is_err()
    );
    assert_eq!(denied.usage().reserved, 0);
    assert_eq!(clipped.coverage(-1, 0), 255);
    assert_eq!(clipped.coverage(-2, 0), 0);
}
#[test]
fn uniform_reframing_and_shared_payload_report_need_no_dense_scan() {
    let huge = Rect::new(0, 0, 1_048_576, 1_048_576).unwrap();
    let budget = SharedBudget::new(0);
    let full = BinaryMask::uniform(huge, true).unwrap();
    assert_eq!(full.storage_usage(), None);
    let smaller = Rect::new(4, 5, 10, 12).unwrap();
    let clipped = full
        .reframed(
            smaller,
            &budget,
            MaskLimits {
                work: 0,
                ..Default::default()
            },
            &|| Ok(()),
        )
        .unwrap();
    assert_eq!(clipped.uniform_coverage(), Some(255));
    let empty = full
        .inverted()
        .reframed(
            Rect::new(-1, -1, 1_048_576, 1_048_576).unwrap(),
            &budget,
            MaskLimits {
                work: 0,
                ..Default::default()
            },
            &|| Ok(()),
        )
        .unwrap();
    assert_eq!(empty.uniform_coverage(), Some(0));
    assert_eq!(budget.usage().peak, 0);
}
