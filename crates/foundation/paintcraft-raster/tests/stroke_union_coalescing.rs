//! Independent geometric union and original-scope lifetime regressions.
use paintcraft_core::types::{
    Error, ErrorCode, Rect,
    resources::{AllocationClass, SharedBudget},
};
use paintcraft_raster::shape::{CheckedSpanSet, Span};
use std::{cell::Cell, collections::BTreeSet, mem::size_of};

fn clip(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn oracle(stamps: &[(i64, i64)], n: i64, round: bool, clip: Rect) -> BTreeSet<(i64, i64)> {
    let mut pixels = BTreeSet::new();
    for y in clip.y0..clip.y1 {
        for x in clip.x0..clip.x1 {
            for &(cx, cy) in stamps {
                let left = i128::from(cx) - (i128::from(n) - 1) / 2;
                let top = i128::from(cy) - (i128::from(n) - 1) / 2;
                let dx = i128::from(x) - left;
                let dy = i128::from(y) - top;
                if dx < 0 || dy < 0 || dx >= i128::from(n) || dy >= i128::from(n) {
                    continue;
                }
                let a = 2 * dx + 1 - i128::from(n);
                let b = 2 * dy + 1 - i128::from(n);
                if !round || a * a + b * b <= i128::from(n) * i128::from(n) {
                    pixels.insert((x, y));
                    break;
                }
            }
        }
    }
    pixels
}
fn actual(stamps: &[(i64, i64)], n: i64, round: bool, clip: Rect) -> BTreeSet<(i64, i64)> {
    let budget = SharedBudget::new(1 << 20);
    let scope = budget.scope(AllocationClass::PaintMetadata);
    let mut set = CheckedSpanSet::with_metadata_scope(1 << 16, scope.clone());
    for &(x, y) in stamps {
        set.add_stamp(x, y, n, round, &clip, &|| Ok(())).unwrap();
    }
    let mut pixels = BTreeSet::new();
    set.visit_spans(&|| Ok(()), |span| {
        for x in span.x0..=span.x1 {
            assert!(
                pixels.insert((x, span.y)),
                "union emits each covered pixel once"
            );
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(budget.usage().reserved, 0);
    pixels
}
#[test]
fn independent_integer_oracle_covers_order_overlap_adjacency_bridges_and_clipping() {
    let points = [(-3, 0), (-1, 0), (0, 0), (1, 0), (3, 0), (0, -1), (0, 2)];
    let clips = [clip(-3, -2, 7, 5), clip(0, 0, 4, 3), clip(-2, -1, 2, 2)];
    for a in points {
        for b in points {
            for c in points {
                let stamps = [a, b, c];
                for n in 1..=5 {
                    for round in [false, true] {
                        for bounds in clips {
                            assert_eq!(
                                actual(&stamps, n, round, bounds),
                                oracle(&stamps, n, round, bounds),
                                "{stamps:?} n={n} round={round} clip={bounds:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn original_257_stamp_rows_fit_same_4096_scope_in_both_directions() {
    for reverse in [false, true] {
        let metadata = SharedBudget::new(4096);
        let scope = metadata.scope(AllocationClass::PaintMetadata);
        let mut set = CheckedSpanSet::with_metadata_scope(4096, scope.clone());
        let checks = Cell::new(0);
        for k in 0..257 {
            let x = if reverse { 256 - k } else { k };
            set.add_stamp(x, 0, 1, false, &clip(0, 0, 257, 1), &|| {
                checks.set(checks.get() + 1);
                Ok(())
            })
            .unwrap();
        }
        // One row map's first allocation plus the original first four-run Vec.
        // Each original input and row still receives its own cancellation check.
        let retained = 3 * size_of::<(i64, Vec<(i64, i64)>)>() + 4 * size_of::<(i64, i64)>();
        assert_eq!(scope.footprint().live, retained as u64);
        assert!(checks.get() >= 2 * 257);
        let mut spans = Vec::new();
        set.visit_spans(&|| Ok(()), |span| {
            spans.push(span);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            spans,
            vec![Span {
                y: 0,
                x0: 0,
                x1: 256
            }]
        );
        assert_eq!(metadata.usage().reserved, 0);
    }
}
#[test]
fn inclusive_boundary_adjacency_does_not_overflow() {
    for (start, bounds) in [
        (i64::MIN, clip(i64::MIN, 0, 4, 1)),
        (i64::MAX - 4, clip(i64::MAX - 4, 0, 4, 1)),
    ] {
        let stamps = [(start + 3, 0), (start + 2, 0), (start + 1, 0), (start, 0)];
        assert_eq!(
            actual(&stamps, 1, false, bounds),
            oracle(&stamps, 1, false, bounds)
        );
    }
}
#[test]
fn every_observed_input_checkpoint_cancels_and_retires_original_scratch() {
    let stamps = [(0, 0), (1, 0), (2, 0), (1, 0), (-1, 0), (3, 1)];
    let bounds = clip(-2, -2, 8, 6);
    let total = Cell::new(0);
    let mut reference = CheckedSpanSet::with_scratch_limit(1 << 16);
    for &(x, y) in &stamps {
        reference
            .add_stamp(x, y, 3, false, &bounds, &|| {
                total.set(total.get() + 1);
                Ok(())
            })
            .unwrap();
    }
    for stop in 1..=total.get() {
        let metadata = SharedBudget::new(1 << 20);
        let scope = metadata.scope(AllocationClass::PaintMetadata);
        let mut set = CheckedSpanSet::with_metadata_scope(1 << 16, scope.clone());
        let calls = Cell::new(0);
        let mut failure = None;
        for &(x, y) in &stamps {
            let result = set.add_stamp(x, y, 3, false, &bounds, &|| {
                calls.set(calls.get() + 1);
                if calls.get() == stop {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "original input checkpoint",
                    ))
                } else {
                    Ok(())
                }
            });
            if let Err(error) = result {
                failure = Some(error);
                break;
            }
        }
        let error = failure.expect("same input/growth checkpoints remain observable");
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(error.message, "original input checkpoint");
        assert_eq!(calls.get(), stop);
        assert_eq!(scope.footprint().live, 0);
        let mut emitted = 0;
        set.visit_spans(&|| Ok(()), |_| {
            emitted += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(emitted, 0);
        assert_eq!(metadata.usage().reserved, 0);
    }
}
#[test]
fn emitter_failure_retires_coalesced_rows_without_relabeling_original_error() {
    let metadata = SharedBudget::new(4096);
    let scope = metadata.scope(AllocationClass::PaintMetadata);
    let mut set = CheckedSpanSet::with_metadata_scope(4096, scope.clone());
    for x in (0..257).rev() {
        set.add_stamp(x, 0, 1, false, &clip(0, 0, 257, 1), &|| Ok(()))
            .unwrap();
    }
    let mut calls = 0;
    let error = set
        .visit_spans(&|| Ok(()), |span| {
            calls += 1;
            assert_eq!(
                span,
                Span {
                    y: 0,
                    x0: 0,
                    x1: 256
                }
            );
            assert!(scope.footprint().live > 0);
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "original union emitter",
            ))
        })
        .unwrap_err();
    assert_eq!(calls, 1);
    assert_eq!(error.code, ErrorCode::OperationCancelled);
    assert_eq!(error.message, "original union emitter");
    assert_eq!(scope.footprint().live, 0);
    assert_eq!(metadata.usage().reserved, 0);
}
