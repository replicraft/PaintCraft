use paintcraft_core::types::{Error, ErrorCode, Rect, resources::SharedBudget, text::MaskSpan};
use paintcraft_raster::selection::{BinaryMask, BoundarySegment, MaskLimits, OutlineLimits};
use std::{cell::Cell, collections::BTreeSet};
type Edge = ((i64, i64), (i64, i64));
fn rect(x: i64, y: i64, w: i64, h: i64) -> Rect {
    Rect::new(x, y, w, h).unwrap()
}
fn mask(bits: u16) -> BinaryMask {
    let spans: Vec<_> = (0..3)
        .flat_map(|y| {
            (0..3)
                .filter(move |x| bits & (1 << (y * 3 + x)) != 0)
                .map(move |x| MaskSpan {
                    y: y - 1,
                    x0: x - 1,
                    x1: x - 1,
                    coverage: 255,
                })
        })
        .collect();
    BinaryMask::from_spans(
        rect(-1, -1, 3, 3),
        false,
        &spans,
        &SharedBudget::new(1 << 20),
        MaskLimits::default(),
        &|| Ok(()),
    )
    .unwrap()
}
fn actual(segments: &[BoundarySegment]) -> BTreeSet<Edge> {
    let mut edges = BTreeSet::new();
    for segment in segments {
        let ((x0, y0), (x1, y1)) = (segment.from, segment.to);
        assert!((x0 == x1 && y0 < y1) || (y0 == y1 && x0 < x1));
        if y0 == y1 {
            for x in x0..x1 {
                assert!(edges.insert(((x, y0), (x + 1, y0))), "duplicate edge");
            }
        } else {
            for y in y0..y1 {
                assert!(edges.insert(((x0, y), (x0, y + 1))), "duplicate edge");
            }
        }
    }
    edges
}
fn oracle(domain: Rect, clip: Rect, selected: impl Fn(i64, i64) -> bool) -> BTreeSet<Edge> {
    let mut edges = BTreeSet::new();
    for y in domain.y0..domain.y1 {
        for x in domain.x0..domain.x1 {
            if !selected(x, y) {
                continue;
            }
            for (nx, ny, edge) in [
                (x - 1, y, ((x, y), (x, y + 1))),
                (x + 1, y, ((x + 1, y), (x + 1, y + 1))),
                (x, y - 1, ((x, y), (x + 1, y))),
                (x, y + 1, ((x, y + 1), (x + 1, y + 1))),
            ] {
                if !selected(nx, ny)
                    && edge.0.0 >= clip.x0
                    && edge.1.0 <= clip.x1
                    && edge.0.1 >= clip.y0
                    && edge.1.1 <= clip.y1
                {
                    edges.insert(edge);
                }
            }
        }
    }
    edges
}
#[test]
fn every_three_by_three_field_matches_independent_pixel_edges_with_holes_and_views() {
    let budget = SharedBudget::new(1 << 20);
    for bits in 0..512 {
        let base = mask(bits);
        for clipped in [false, true] {
            let view = if clipped {
                base.clipped(rect(-1, 0, 2, 2)).unwrap().unwrap()
            } else {
                base.clone()
            };
            for inverted in [false, true] {
                let view = if inverted {
                    view.inverted()
                } else {
                    view.clone()
                };
                for (dx, dy) in [(0, 0), (-3, 2)] {
                    let view = view.translated(dx, dy).unwrap();
                    let domain = view.domain();
                    for clip in [
                        rect(-7, -4, 14, 12),
                        rect(dx, dy, 1, 1),
                        rect(dx + 2, dy - 2, 1, 5),
                    ] {
                        let outline = view
                            .outline_checked(clip, &budget, OutlineLimits::default(), &|| Ok(()))
                            .unwrap();
                        let selected = |x: i64, y: i64| {
                            if !domain.contains(x, y) {
                                return false;
                            }
                            let (sx, sy) = (x - dx + 1, y - dy + 1);
                            ((bits & (1 << (sy * 3 + sx))) != 0) ^ inverted
                        };
                        assert_eq!(
                            actual(outline.segments()),
                            oracle(domain, clip, selected),
                            "bits={bits} clip={clip:?} domain={domain:?} invert={inverted}"
                        );
                    }
                }
            }
        }
    }
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn uniform_huge_fields_need_constant_work_and_do_not_close_the_viewport() {
    let huge = rect(-500_000, -500_000, 1_000_000, 1_000_000);
    let mask = BinaryMask::uniform(huge, true).unwrap();
    let budget = SharedBudget::new(1024);
    let calls = Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let all = mask
        .outline_checked(
            huge,
            &budget,
            OutlineLimits {
                work: 4,
                segments: 4,
            },
            &check,
        )
        .unwrap();
    assert_eq!(all.segments().len(), 4);
    assert!(calls.get() <= 6);
    assert!(budget.usage().reserved >= 4 * 32);
    drop(all);
    let inside = mask
        .outline_checked(
            rect(10, 20, 100, 80),
            &budget,
            OutlineLimits {
                work: 4,
                segments: 0,
            },
            &|| Ok(()),
        )
        .unwrap();
    assert!(inside.segments().is_empty());
    let corner = mask
        .outline_checked(
            rect(499_990, 499_990, 20, 20),
            &budget,
            OutlineLimits {
                work: 4,
                segments: 2,
            },
            &|| Ok(()),
        )
        .unwrap();
    assert_eq!(corner.segments().len(), 2);
    drop(corner);
    drop(inside);
    assert_eq!(budget.usage().reserved, 0);
    let empty = mask
        .inverted()
        .outline_checked(
            huge,
            &SharedBudget::new(0),
            OutlineLimits {
                work: 0,
                segments: 0,
            },
            &|| Ok(()),
        )
        .unwrap();
    assert!(empty.segments().is_empty());
}
#[test]
fn viewport_cuts_do_not_invent_edges_and_true_edges_on_clip_perimeter_survive() {
    let m = mask(0b111_101_111);
    let clip = rect(0, -1, 2, 3);
    let geometry = m
        .outline_checked(
            clip,
            &SharedBudget::new(1 << 20),
            OutlineLimits::default(),
            &|| Ok(()),
        )
        .unwrap();
    assert_eq!(
        actual(geometry.segments()),
        oracle(m.domain(), clip, |x, y| m.domain().contains(x, y)
            && (x != 0 || y != 0))
    );
    let contact = rect(2, -1, 1, 3);
    let edge = m
        .outline_checked(
            contact,
            &SharedBudget::new(1 << 20),
            OutlineLimits::default(),
            &|| Ok(()),
        )
        .unwrap();
    assert_eq!(
        actual(edge.segments()),
        BTreeSet::from([((2, -1), (2, 0)), ((2, 0), (2, 1)), ((2, 1), (2, 2))])
    );
}
#[test]
fn every_observed_cancellation_and_all_denials_release_private_geometry() {
    let mask = mask(0b101_010_101);
    let clip = rect(-2, -2, 5, 5);
    let budget = SharedBudget::new(1 << 20);
    let calls = Cell::new(0);
    drop(
        mask.outline_checked(clip, &budget, OutlineLimits::default(), &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap(),
    );
    for at in 1..=calls.get() {
        let count = Cell::new(0);
        let err = mask
            .outline_checked(clip, &budget, OutlineLimits::default(), &|| {
                count.set(count.get() + 1);
                if count.get() == at {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel outline"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::OperationCancelled);
        assert_eq!(budget.usage().reserved, 0);
    }
    for limits in [
        OutlineLimits {
            work: 0,
            segments: 1024,
        },
        OutlineLimits {
            work: 100_000,
            segments: 1,
        },
    ] {
        assert_eq!(
            mask.outline_checked(clip, &budget, limits, &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(budget.usage().reserved, 0);
    }
    for bytes in [0, 64, 256] {
        let denied = SharedBudget::new(bytes);
        assert_eq!(
            mask.outline_checked(clip, &denied, OutlineLimits::default(), &|| Ok(()))
                .unwrap_err()
                .code,
            ErrorCode::ResourceLimit
        );
        assert_eq!(denied.usage().reserved, 0);
    }
    assert_eq!(mask.coverage(-1, -1), 255);
    assert_eq!(mask.coverage(0, -1), 0);
}
