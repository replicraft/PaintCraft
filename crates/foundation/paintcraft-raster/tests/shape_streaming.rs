use paintcraft_core::types::{Error, ErrorCode, Rect};
use paintcraft_raster::shape::{self, Figure, Span};
use std::{cell::Cell, collections::BTreeSet};
fn cells(spans: &[Span]) -> BTreeSet<(i64, i64)> {
    let mut out = BTreeSet::new();
    for span in spans {
        for x in span.x0..=span.x1 {
            assert!(out.insert((x, span.y)));
        }
    }
    out
}
// Independent schoolbook u32 limbs, rather than the production four u64 products.
fn mul(a: u128, b: u128) -> [u32; 8] {
    let mut out = [0u32; 8];
    for i in 0..4 {
        let a = (a >> (32 * i)) as u32;
        let mut carry = 0u64;
        for j in 0..4 {
            let b = (b >> (32 * j)) as u32;
            let n = u64::from(a) * u64::from(b) + u64::from(out[i + j]) + carry;
            out[i + j] = n as u32;
            carry = n >> 32;
        }
        out[i + 4] = carry as u32;
    }
    out
}
fn add(a: [u32; 8], b: [u32; 8]) -> [u32; 8] {
    let mut out = [0u32; 8];
    let mut carry = 0u64;
    for i in 0..8 {
        let n = u64::from(a[i]) + u64::from(b[i]) + carry;
        out[i] = n as u32;
        carry = n >> 32;
    }
    assert_eq!(carry, 0);
    out
}
fn inside(fig: Figure, bbox: Rect, x: i64, y: i64) -> bool {
    if !bbox.contains(x, y) {
        return false;
    }
    if fig == Figure::Rect {
        return true;
    }
    let (w, h) = (bbox.width() as u128, bbox.height() as u128);
    let u = (2 * (i128::from(x) - i128::from(bbox.x0)) + 1 - w as i128).unsigned_abs();
    let v = (2 * (i128::from(y) - i128::from(bbox.y0)) + 1 - h as i128).unsigned_abs();
    let left = add(mul(u * u, h * h), mul(v * v, w * w));
    let right = mul(w * w, h * h);
    left.iter().rev().cmp(right.iter().rev()) != std::cmp::Ordering::Greater
}
fn oracle(fig: Figure, bbox: Rect, clip: Rect, t: Option<i64>) -> BTreeSet<(i64, i64)> {
    let inner = t.and_then(|t| {
        if i128::from(bbox.width()) > 2 * i128::from(t)
            && i128::from(bbox.height()) > 2 * i128::from(t)
        {
            Rect::new(
                bbox.x0 + t,
                bbox.y0 + t,
                bbox.width() - 2 * t,
                bbox.height() - 2 * t,
            )
        } else {
            None
        }
    });
    let mut result = BTreeSet::new();
    for y in clip.y0..clip.y1 {
        for x in clip.x0..clip.x1 {
            if inside(fig, bbox, x, y) && !inner.is_some_and(|r| inside(fig, r, x, y)) {
                result.insert((x, y));
            }
        }
    }
    result
}
fn visit(fig: Figure, bbox: Rect, clip: Rect, t: Option<i64>) -> Vec<Span> {
    let mut out = Vec::new();
    if let Some(t) = t {
        shape::visit_outline(fig, &bbox, t, &clip, &|| Ok(()), |s| {
            out.push(s);
            Ok(())
        })
        .unwrap();
    } else {
        shape::visit_filled(fig, &bbox, &clip, &|| Ok(()), |s| {
            out.push(s);
            Ok(())
        })
        .unwrap();
    }
    out
}
#[test]
fn small_shapes_match_independent_pixel_centres_and_inset_difference() {
    for w in 1..=12 {
        for h in 1..=12 {
            for fig in [Figure::Rect, Figure::Ellipse] {
                let bbox = Rect::new(-3, -2, w, h).unwrap();
                for clip in [
                    Rect::new(-5, -4, 16, 16).unwrap(),
                    Rect::new(0, 0, 4, 3).unwrap(),
                ] {
                    for t in [None, Some(0), Some(1), Some(3), Some(16)] {
                        let spans = visit(fig, bbox, clip, t);
                        assert_eq!(
                            cells(&spans),
                            oracle(fig, bbox, clip, t),
                            "{fig:?}, {w}x{h}, t={t:?}"
                        );
                        let old = if let Some(t) = t {
                            shape::outline(fig, &bbox, t, &clip)
                        } else {
                            shape::filled(fig, &bbox, &clip)
                        };
                        assert_eq!(spans, old);
                    }
                }
            }
        }
    }
}
#[test]
fn huge_clipped_ellipses_match_independent_wide_inequality_at_centres_and_edges() {
    let w = (1i64 << 47) - 3;
    let h = (1i64 << 46) + 1;
    let bbox = Rect::new(0, 0, w, h).unwrap();
    // Locate the first pixel independently, then inspect both sides of that edge.
    let (mut lo, mut hi) = (0, w / 2);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if inside(Figure::Ellipse, bbox, mid, 1) {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    let max = i64::MAX;
    let huge = Rect::new(-max / 2, -max / 2, max, max).unwrap();
    for (bbox, clip) in [
        (bbox, Rect::new(lo - 3, 1, 8, 4).unwrap()),
        (huge, Rect::new(-3, -3, 7, 7).unwrap()),
        (huge, Rect::new(huge.x0 - 2, -3, 7, 7).unwrap()),
    ] {
        for t in [None, Some(0), Some(1), Some(3), Some(i64::MAX)] {
            assert_eq!(
                cells(&visit(Figure::Ellipse, bbox, clip, t)),
                oracle(Figure::Ellipse, bbox, clip, t)
            );
        }
    }
}
#[test]
fn cancellation_and_sink_failure_stop_before_materializing_large_geometry() {
    let bbox = Rect::new(0, 0, 2, 1_000_000).unwrap();
    for fig in [Figure::Rect, Figure::Ellipse] {
        for t in [None, Some(1)] {
            let checks = Cell::new(0);
            let emitted = Cell::new(0);
            let check = || {
                checks.set(checks.get() + 1);
                if checks.get() == 5 {
                    Err(Error::new(ErrorCode::OperationCancelled, "cancel geometry"))
                } else {
                    Ok(())
                }
            };
            let emit = |_: Span| {
                emitted.set(emitted.get() + 1);
                Ok(())
            };
            let error = if let Some(t) = t {
                shape::visit_outline(fig, &bbox, t, &bbox, &check, emit)
            } else {
                shape::visit_filled(fig, &bbox, &bbox, &check, emit)
            }
            .unwrap_err();
            assert_eq!(error.code, ErrorCode::OperationCancelled);
            assert_eq!(checks.get(), 5);
            assert!(emitted.get() <= 6);
        }
    }
    let emitted = Cell::new(0);
    let error = shape::visit_filled(Figure::Rect, &bbox, &bbox, &|| Ok(()), |_| {
        emitted.set(emitted.get() + 1);
        if emitted.get() == 3 {
            Err(Error::new(ErrorCode::ResourceLimit, "sink full").at(17))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(error.message, "sink full");
    assert_eq!(error.operation_index, Some(17));
    assert_eq!(emitted.get(), 3);
}
#[test]
fn visitors_check_numeric_bounds_and_handle_signed_endpoints() {
    let clip = Rect::new(i64::MIN, i64::MIN, 3, 3).unwrap();
    for fig in [Figure::Rect, Figure::Ellipse] {
        for t in [None, Some(0), Some(1)] {
            assert_eq!(
                cells(&visit(fig, clip, clip, t)),
                oracle(fig, clip, clip, t)
            );
        }
    }
    let bad = Rect {
        x0: i64::MIN,
        x1: i64::MAX,
        y0: 0,
        y1: 1,
    };
    assert_eq!(
        shape::visit_filled(Figure::Ellipse, &bad, &clip, &|| Ok(()), |_| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::ResourceLimit
    );
    assert_eq!(
        shape::visit_outline(Figure::Rect, &clip, -1, &clip, &|| Ok(()), |_| Ok(()))
            .unwrap_err()
            .code,
        ErrorCode::InputSchema
    );
}
