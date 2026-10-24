//! Integer-exact raster geometry as horizontal spans: filled and outlined
//! rectangles and ellipses, and round or square brush stamps. Everything is
//! decided with integer arithmetic so every implementation draws identical pixels.
//!
//! An ellipse inscribed in a `w × h` box contains pixel `(px, py)` exactly when
//! `(2px+1-w)²·h² + (2py+1-h)²·w² ≤ w²·h²`, the pixel centre test. A round brush of
//! diameter `n` is the `n × n` ellipse, a square brush is the full `n × n` box.

use std::collections::HashMap;

use paintcraft_core::types::{Error, ErrorCode, Rect, Result};
mod checked;
mod ellipse;
mod polygon;
pub mod presets;
mod stroke_budget;
pub use checked::{CheckedSpanSet, DEFAULT_STROKE_SCRATCH_BYTES};
pub use polygon::visit_polygon;

/// Inclusive horizontal run on absolute row `y`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    pub y: i64,
    pub x0: i64,
    pub x1: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Figure {
    Rect,
    Ellipse,
}

fn ellipse_row(w: i64, h: i64, py: i64) -> Option<(i64, i64)> {
    ellipse::row(w, h, py)
}

/// Filled figure inside `bbox`, local row range of box row `py` (inclusive).
fn row_range(fig: Figure, w: i64, h: i64, py: i64) -> Option<(i64, i64)> {
    if !(0..h).contains(&py) {
        return None;
    }
    match fig {
        Figure::Rect => Some((0, w - 1)),
        Figure::Ellipse => ellipse_row(w, h, py),
    }
}

fn clip_span(
    y: i64,
    x0: i64,
    x1: i64,
    clip: &Rect,
    emit: &mut impl FnMut(Span) -> Result<()>,
) -> Result<()> {
    let (x0, x1) = (x0.max(clip.x0), x1.min(clip.x1 - 1));
    if x0 <= x1 && (clip.y0..clip.y1).contains(&y) {
        emit(Span { y, x0, x1 })?;
    }
    Ok(())
}
fn dimensions(bbox: &Rect) -> Result<(i64, i64)> {
    let extent = |a: i64, b: i64| {
        b.checked_sub(a).filter(|n| *n > 0).ok_or_else(|| {
            Error::new(
                ErrorCode::ResourceLimit,
                "shape extent exceeds numeric limits",
            )
        })
    };
    Ok((extent(bbox.x0, bbox.x1)?, extent(bbox.y0, bbox.y1)?))
}

/// Stream exact clipped fill spans into a fallible sink. Checks cancellation for
/// every row; no geometry/output vector is retained by the visitor.
pub fn visit_filled(
    fig: Figure,
    bbox: &Rect,
    clip: &Rect,
    check: &dyn Fn() -> Result<()>,
    mut emit: impl FnMut(Span) -> Result<()>,
) -> Result<()> {
    check()?;
    let (w, h) = dimensions(bbox)?;
    let Some(rows) = bbox.intersect(clip) else {
        return Ok(());
    };
    for y in rows.y0..rows.y1 {
        check()?;
        if let Some((a, b)) = row_range(fig, w, h, y - bbox.y0) {
            clip_span(y, bbox.x0 + a, bbox.x0 + b, clip, &mut emit)?;
        }
    }
    check()
}
/// Stream the exact outline (outer figure minus the inset figure). A zero
/// thickness emits nothing; negative thickness is invalid.
pub fn visit_outline(
    fig: Figure,
    bbox: &Rect,
    t: i64,
    clip: &Rect,
    check: &dyn Fn() -> Result<()>,
    mut emit: impl FnMut(Span) -> Result<()>,
) -> Result<()> {
    check()?;
    if t < 0 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "negative outline thickness",
        ));
    }
    let (w, h) = dimensions(bbox)?;
    let twice = 2 * i128::from(t);
    let inner = (i128::from(w) > twice && i128::from(h) > twice).then(|| {
        (
            (i128::from(w) - twice) as i64,
            (i128::from(h) - twice) as i64,
        )
    });
    let Some(rows) = bbox.intersect(clip) else {
        return Ok(());
    };
    for y in rows.y0..rows.y1 {
        check()?;
        let py = y - bbox.y0;
        let Some((a, b)) = row_range(fig, w, h, py) else {
            continue;
        };
        let hole = inner
            .and_then(|(iw, ih)| row_range(fig, iw, ih, py - t))
            .map(|(c, d)| (c + t, d + t));
        if let Some((c, d)) = hole {
            // Test empty local pieces before adding the world origin. This also
            // avoids constructing x0-1 at i64::MIN for an empty left piece.
            if a < c {
                clip_span(y, bbox.x0 + a, bbox.x0 + b.min(c - 1), clip, &mut emit)?;
            }
            if d < b {
                clip_span(y, bbox.x0 + a.max(d + 1), bbox.x0 + b, clip, &mut emit)?;
            }
        } else {
            clip_span(y, bbox.x0 + a, bbox.x0 + b, clip, &mut emit)?;
        }
    }
    check()
}

/// Collect fill spans for callers that own their geometry allocation policy.
pub fn filled(fig: Figure, bbox: &Rect, clip: &Rect) -> Vec<Span> {
    let mut out = Vec::new();
    visit_filled(fig, bbox, clip, &|| Ok(()), |span| {
        out.push(span);
        Ok(())
    })
    .expect("valid shape bounds");
    out
}
/// Collect outline spans of nonnegative thickness. For fallible allocation and
/// cooperative work, prefer visit_outline with an admitted sink.
pub fn outline(fig: Figure, bbox: &Rect, t: i64, clip: &Rect) -> Vec<Span> {
    let mut out = Vec::new();
    visit_outline(fig, bbox, t, clip, &|| Ok(()), |span| {
        out.push(span);
        Ok(())
    })
    .expect("valid shape bounds and thickness");
    out
}

/// Union of spans accumulated from many brush stamps, merged so that painting
/// each resulting span once blends every covered pixel exactly once.
#[derive(Debug, Default)]
pub struct SpanSet {
    rows: HashMap<i64, Vec<(i64, i64)>>,
}

impl SpanSet {
    /// Add one stamp of diameter `n` whose centre pixel is `(cx, cy)`.
    pub fn add_stamp(&mut self, cx: i64, cy: i64, n: i64, round: bool, clip: &Rect) {
        let (left, top) = (cx - (n - 1) / 2, cy - (n - 1) / 2);
        let first = (clip.y0 - top).max(0);
        let last = (clip.y1 - top).min(n);
        for j in first..last {
            let range = if round {
                ellipse_row(n, n, j)
            } else {
                Some((0, n - 1))
            };
            if let Some((a, b)) = range {
                let (x0, x1) = ((left + a).max(clip.x0), (left + b).min(clip.x1 - 1));
                if x0 <= x1 {
                    self.rows.entry(top + j).or_default().push((x0, x1));
                }
            }
        }
    }

    pub fn into_spans(self) -> Vec<Span> {
        let mut ys: Vec<_> = self.rows.into_iter().collect();
        ys.sort_unstable_by_key(|(y, _)| *y);
        let mut out = Vec::new();
        for (y, mut runs) in ys {
            runs.sort_unstable();
            let mut cur = runs[0];
            for &(a, b) in &runs[1..] {
                if a <= cur.1 + 1 {
                    cur.1 = cur.1.max(b);
                } else {
                    out.push(Span {
                        y,
                        x0: cur.0,
                        x1: cur.1,
                    });
                    cur = (a, b);
                }
            }
            out.push(Span {
                y,
                x0: cur.0,
                x1: cur.1,
            });
        }
        out
    }
}

#[cfg(test)]
pub(crate) fn render(spans: &[Span], w: usize, h: usize) -> Vec<String> {
    let mut g = vec![vec!['.'; w]; h];
    for s in spans {
        for x in s.x0..=s.x1 {
            g[s.y as usize][x as usize] = '#';
        }
    }
    g.into_iter().map(|r| r.into_iter().collect()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big() -> Rect {
        Rect::new(-100, -100, 1000, 1000).unwrap()
    }

    #[test]
    fn ellipse_pixels_follow_the_centre_test_exactly() {
        for (w, h) in [(1, 1), (2, 2), (3, 3), (5, 3), (4, 7), (9, 9), (16, 5)] {
            for py in 0..h {
                for px in 0..w {
                    let (u, v) = (i128::from(2 * px + 1 - w), i128::from(2 * py + 1 - h));
                    let (w2, h2) = (i128::from(w), i128::from(h));
                    let inside = u * u * h2 * h2 + v * v * w2 * w2 <= w2 * w2 * h2 * h2;
                    let got = ellipse_row(w, h, py).is_some_and(|(a, b)| (a..=b).contains(&px));
                    assert_eq!(got, inside, "{w}x{h} at {px},{py}");
                }
            }
        }
    }

    #[test]
    fn small_figures_have_the_expected_shape() {
        let bbox = Rect::new(0, 0, 5, 5).unwrap();
        assert_eq!(
            render(
                &filled(
                    Figure::Ellipse,
                    &bbox,
                    &big().intersect(&Rect::new(0, 0, 5, 5).unwrap()).unwrap()
                ),
                5,
                5
            ),
            [".###.", "#####", "#####", "#####", ".###."]
        );
        let clip = Rect::new(0, 0, 6, 6).unwrap();
        assert_eq!(
            render(
                &outline(Figure::Rect, &Rect::new(0, 0, 5, 4).unwrap(), 1, &clip),
                6,
                6
            )[..4],
            ["#####.", "#...#.", "#...#.", "#####."]
        );
        assert_eq!(
            render(&outline(Figure::Ellipse, &bbox, 1, &clip), 6, 6)[..5],
            [".###..", "#...#.", "#...#.", "#...#.", ".###.."]
        );
        // Thick outlines on a small box are solid.
        assert_eq!(
            render(
                &outline(Figure::Rect, &Rect::new(0, 0, 3, 3).unwrap(), 2, &clip),
                6,
                6
            )[..3],
            ["###...", "###...", "###..."]
        );
    }

    #[test]
    fn clipping_never_widens_and_far_away_shapes_are_cheap() {
        let clip = Rect::new(2, 2, 3, 3).unwrap();
        for s in filled(
            Figure::Rect,
            &Rect::new(
                -1_000_000_000_000,
                -1_000_000_000_000,
                3_000_000_000_000,
                3_000_000_000_000,
            )
            .unwrap(),
            &clip,
        ) {
            assert!(clip.contains(s.x0, s.y) && clip.contains(s.x1, s.y));
        }
        assert!(filled(Figure::Ellipse, &Rect::new(100, 100, 5, 5).unwrap(), &clip).is_empty());
    }

    #[test]
    fn brush_stamps_cover_pixels_once_after_merging() {
        let clip = Rect::new(0, 0, 20, 20).unwrap();
        let mut set = SpanSet::default();
        for x in 3..=8 {
            set.add_stamp(x, 5, 3, false, &clip);
        }
        let spans = set.into_spans();
        assert_eq!(
            spans,
            [
                Span { y: 4, x0: 2, x1: 9 },
                Span { y: 5, x0: 2, x1: 9 },
                Span { y: 6, x0: 2, x1: 9 }
            ]
        );
        let mut round = SpanSet::default();
        round.add_stamp(5, 5, 5, true, &clip);
        assert_eq!(
            render(&round.into_spans(), 10, 10)[3..8],
            [
                "....###...",
                "...#####..",
                "...#####..",
                "...#####..",
                "....###..."
            ]
        );
        // Even diameters extend one pixel further right/down of the centre.
        let mut even = SpanSet::default();
        even.add_stamp(5, 5, 2, false, &clip);
        assert_eq!(
            even.into_spans(),
            [Span { y: 5, x0: 5, x1: 6 }, Span { y: 6, x0: 5, x1: 6 }]
        );
    }
}
