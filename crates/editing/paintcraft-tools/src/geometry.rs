//! Live Shift constraints share one endpoint calculation for preview and commit.
use super::Tool;

type Point = (i64, i64);

pub(super) fn endpoints(tool: Tool, points: &[Point], constrained: bool) -> (Point, Point) {
    let first = points[0];
    let last = *points.last().expect("a gesture has a point");
    if !constrained || !matches!(tool, Tool::Line | Tool::Rect | Tool::Ellipse) {
        return (first, last);
    }
    let dx = last.0.saturating_sub(first.0);
    let dy = last.1.saturating_sub(first.1);
    let (w, h) = (dx.saturating_abs(), dy.saturating_abs());
    let side = w.max(h);
    let (w, h) = if tool == Tool::Line && i128::from(h) * 1000 <= i128::from(w) * 414 {
        (w, 0)
    } else if tool == Tool::Line && i128::from(w) * 1000 <= i128::from(h) * 414 {
        (0, h)
    } else {
        (side, side)
    };
    (
        first,
        (
            first.0.saturating_add(if dx < 0 { -w } else { w }),
            first.1.saturating_add(if dy < 0 { -h } else { h }),
        ),
    )
}
