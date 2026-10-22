//! Borrowed segment traversal; off-canvas work must remain cancellable.
use super::*;
pub(super) fn segments(
    points: &[(i64, i64)],
) -> impl Iterator<Item = ((i64, i64), (i64, i64))> + '_ {
    (points.len() == 1)
        .then(|| (points[0], points[0]))
        .into_iter()
        .chain(points.windows(2).map(|w| (w[0], w[1])))
}
pub(super) fn length(points: &[(i64, i64)], check: &dyn Fn() -> Result<()>) -> Result<i128> {
    length_scaled(points, 1, check)
}
pub(super) fn length_scaled(
    points: &[(i64, i64)],
    scale: u32,
    check: &dyn Fn() -> Result<()>,
) -> Result<i128> {
    if scale == 0 {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "point scale must be positive",
        ));
    }
    let scale = i64::from(scale);
    let mut length = 0i128;
    for (index, w) in points.windows(2).enumerate() {
        if index.is_multiple_of(1024) {
            check()?;
        }
        let dx =
            (i128::from(w[1].0.div_euclid(scale)) - i128::from(w[0].0.div_euclid(scale))).abs();
        let dy =
            (i128::from(w[1].1.div_euclid(scale)) - i128::from(w[0].1.div_euclid(scale))).abs();
        length = length
            .checked_add(dx.max(dy))
            .ok_or_else(|| Error::new(ErrorCode::ResourceLimit, "path length overflows"))?;
    }
    check()?;
    Ok(length)
}
/// The symmetric all-octant Bresenham recurrence, yielding only pixels inside
/// `clip`. The walk is monotonic, so it stops once the remainder cannot re-enter
/// the clip box. Every visited pixel is exactly one the unclipped walk emits.
pub(super) fn walk_clipped(
    a: (i64, i64),
    b: (i64, i64),
    clip: Option<Rect>,
    skip_first: bool,
    check: &dyn Fn() -> Result<()>,
    mut emit: impl FnMut(i64, i64) -> Result<()>,
) -> Result<()> {
    check()?;
    let Some(clip) = clip else { return Ok(()) };
    let (mut x, mut y) = a;
    let (x1, y1) = b;
    let dx = (i128::from(x1) - i128::from(x)).abs();
    let sx = if x < x1 { 1 } else { -1 };
    let dy = -(i128::from(y1) - i128::from(y)).abs();
    let sy = if y < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut first = true;
    let mut work = 0usize;
    loop {
        if work.is_multiple_of(1024) {
            check()?;
        }
        work += 1;
        if x.min(x1) >= clip.x1
            || x.max(x1) < clip.x0
            || y.min(y1) >= clip.y1
            || y.max(y1) < clip.y0
        {
            return Ok(());
        }
        if !(first && skip_first) && clip.contains(x, y) {
            emit(x, y)?;
        }
        first = false;
        if (x, y) == (x1, y1) {
            return Ok(());
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn invisible_walk_checks_before_ever_reaching_the_clip() {
        let calls = Cell::new(0);
        let emitted = Cell::new(0);
        let error = walk_clipped(
            (-1_000_000, 0),
            (1_000_000, 0),
            Rect::new(0, 0, 8, 8),
            false,
            &|| {
                calls.set(calls.get() + 1);
                if calls.get() == 3 {
                    Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "cancel invisible prefix",
                    ))
                } else {
                    Ok(())
                }
            },
            |_, _| {
                emitted.set(emitted.get() + 1);
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
        assert_eq!(emitted.get(), 0);
    }
    #[test]
    fn borrowed_segments_and_cancellable_wide_length_keep_original_joint_policy() {
        assert_eq!(segments(&[]).count(), 0);
        assert_eq!(
            segments(&[(1, 2)]).collect::<Vec<_>>(),
            vec![((1, 2), (1, 2))]
        );
        assert_eq!(
            segments(&[(1, 2), (3, 4), (5, 6)]).collect::<Vec<_>>(),
            vec![((1, 2), (3, 4)), ((3, 4), (5, 6))]
        );
        assert_eq!(
            length(&[(i64::MIN, 0), (i64::MAX, 0)], &|| Ok(())).unwrap(),
            i128::from(u64::MAX)
        );
        assert_eq!(
            length_scaled(&[(-257, 0), (-1, 255), (257, 257)], 256, &|| Ok(())).unwrap(),
            3
        );
        assert_eq!(
            length_scaled(&[(0, 0)], 0, &|| Ok(())).unwrap_err().code,
            ErrorCode::InputSchema
        );
        let points = vec![(0, 0); 10_000];
        let seen = Cell::new(0);
        let error = length(&points, &|| {
            seen.set(seen.get() + 1);
            if seen.get() == 3 {
                Err(Error::new(ErrorCode::OperationCancelled, "cancel length"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::OperationCancelled);
    }
}
