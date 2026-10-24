//! Clipped even-odd polygon fills. Pixel centres use a half-open edge rule.
use super::Span;
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, policy::COORD_ABS_MAX};

pub fn visit_polygon(
    points: &[(i64, i64)],
    clip: &Rect,
    check: &dyn Fn() -> Result<()>,
    mut emit: impl FnMut(Span) -> Result<()>,
) -> Result<()> {
    check()?;
    if points.len() < 3 || points.len() > 1024 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "polygon needs 3..=1024 vertices",
        ));
    }
    if points.iter().any(|&(x, y)| {
        x.unsigned_abs() > COORD_ABS_MAX as u64 || y.unsigned_abs() > COORD_ABS_MAX as u64
    }) {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "polygon coordinate exceeds policy",
        ));
    }
    let first = points.iter().map(|p| p.1).min().unwrap().max(clip.y0);
    let last = points.iter().map(|p| p.1).max().unwrap().min(clip.y1);
    if first >= last || clip.x0 >= clip.x1 {
        return Ok(());
    }
    if i128::from(last - first) * points.len() as i128 > 16_777_216 {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "polygon edge-test budget exceeded",
        ));
    }
    let mut crossings = Vec::new();
    crossings.try_reserve_exact(points.len()).map_err(|_| {
        Error::new(
            ErrorCode::ResourceLimit,
            "cannot reserve polygon intersections",
        )
    })?;
    for y in first..last {
        check()?;
        crossings.clear();
        for (index, (&a, &b)) in points.iter().zip(points.iter().cycle().skip(1)).enumerate() {
            if index.is_multiple_of(32) {
                check()?;
            }
            let (a, b) = if a.1 <= b.1 { (a, b) } else { (b, a) };
            if y < a.1 || y >= b.1 {
                continue;
            }
            let dy = i128::from(b.1 - a.1);
            let dx = i128::from(b.0 - a.0);
            // ceil(intersection_x - 1/2), computed exactly before rounding.
            // Policy coordinates are <2^47, keeping intermediates within i128.
            let denominator = 2 * dy;
            let numerator = 2 * i128::from(a.0) * dy + (2 * i128::from(y - a.1) + 1) * dx - dy;
            let x = numerator.div_euclid(denominator)
                + i128::from(numerator.rem_euclid(denominator) != 0);
            crossings.push(x as i64);
        }
        crossings.sort_unstable();
        for pair in crossings.as_chunks::<2>().0 {
            let x0 = pair[0].max(clip.x0);
            let x1 = pair[1].min(clip.x1) - 1;
            if x0 <= x1 {
                emit(Span { y, x0, x1 })?;
            }
        }
    }
    check()
}
