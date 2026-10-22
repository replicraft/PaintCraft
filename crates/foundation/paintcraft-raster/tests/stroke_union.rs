use paintcraft_core::types::{Error, ErrorCode, Rect};
use paintcraft_raster::shape::{CheckedSpanSet, Span, SpanSet};
use std::{
    cell::Cell,
    collections::{BTreeSet, HashMap},
    mem::size_of,
};
fn cells(spans: &[Span]) -> BTreeSet<(i64, i64)> {
    let mut out = BTreeSet::new();
    for s in spans {
        for x in s.x0..=s.x1 {
            assert!(out.insert((x, s.y)), "pixel emitted twice");
        }
    }
    out
}
#[test]
fn checked_union_matches_independent_constant_colour_stamp_coverage() {
    let clip = Rect::new(-4, -3, 20, 18).unwrap();
    let centres = [(-2, 0), (3, 5), (9, 8), (3, 5), (-2, 0)];
    for n in 1..=16 {
        for round in [false, true] {
            let mut set = CheckedSpanSet::with_scratch_limit(1 << 20);
            let mut old = SpanSet::default();
            for (x, y) in centres {
                set.add_stamp(x, y, n, round, &clip, &|| Ok(())).unwrap();
                old.add_stamp(x, y, n, round, &clip);
            }
            let mut spans = Vec::new();
            set.visit_spans(&|| Ok(()), |s| {
                spans.push(s);
                Ok(())
            })
            .unwrap();
            let mut oracle = BTreeSet::new();
            for y in clip.y0..clip.y1 {
                for x in clip.x0..clip.x1 {
                    if centres.iter().any(|&(cx, cy)| {
                        let (px, py) = (x - (cx - (n - 1) / 2), y - (cy - (n - 1) / 2));
                        if !(0..n).contains(&px) || !(0..n).contains(&py) {
                            return false;
                        }
                        let (u, v, n) = (
                            i128::from(2 * px + 1 - n),
                            i128::from(2 * py + 1 - n),
                            i128::from(n),
                        );
                        !round || u * u * n * n + v * v * n * n <= n * n * n * n
                    }) {
                        oracle.insert((x, y));
                    }
                }
            }
            assert_eq!(cells(&spans), oracle);
            assert_eq!(cells(&old.into_spans()), oracle);
        }
    }
}
fn scrambled(limit: usize) -> CheckedSpanSet {
    let mut set = CheckedSpanSet::with_scratch_limit(limit);
    let clip = Rect::new(0, 0, 4096, 3).unwrap();
    for i in 0..4096 {
        set.add_stamp((i * 37) % 4096, 1, 1, false, &clip, &|| Ok(()))
            .unwrap();
    }
    set
}
#[test]
fn sorted_chunks_merge_exactly_and_cancel_at_every_observed_boundary() {
    let calls = Cell::new(0);
    let mut spans = Vec::new();
    scrambled(1 << 20)
        .visit_spans(
            &|| {
                calls.set(calls.get() + 1);
                Ok(())
            },
            |s| {
                spans.push(s);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(
        spans,
        vec![Span {
            y: 1,
            x0: 0,
            x1: 4095
        }]
    );
    assert!(calls.get() > 10);
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let error = scrambled(1 << 20)
            .visit_spans(
                &|| {
                    seen.set(seen.get() + 1);
                    if seen.get() == stop {
                        Err(Error::new(ErrorCode::OperationCancelled, "cancel union"))
                    } else {
                        Ok(())
                    }
                },
                |_| Ok(()),
            )
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(seen.get(), stop);
    }
}
#[test]
fn row_vectors_and_merge_heap_share_one_fixed_allowance() {
    let clip = Rect::new(0, 0, 4096, 3).unwrap();
    let mut none = CheckedSpanSet::with_scratch_limit(0);
    let error = none
        .add_stamp(1, 1, 1, true, &clip, &|| Ok(()))
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::ResourceLimit);
    assert!(error.allocation_denial().is_none());
    let mut table = HashMap::<i64, Vec<(i64, i64)>>::new();
    table.try_reserve(1).unwrap();
    let limit =
        table.capacity() * size_of::<(i64, Vec<(i64, i64)>)>() + 4096 * size_of::<(i64, i64)>();
    let set = scrambled(limit);
    let emitted = Cell::new(0);
    let error = set
        .visit_spans(&|| Ok(()), |_| {
            emitted.set(emitted.get() + 1);
            Ok(())
        })
        .unwrap_err();
    assert_eq!(error.message, "stroke union scratch budget exceeded");
    assert!(error.allocation_denial().is_none());
    assert_eq!(emitted.get(), 0);
}
