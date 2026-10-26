use super::*;
use paintcraft_core::types::{
    Rect,
    policy::COORD_ABS_MAX,
    resources::{AllocationClass, BudgetAccount, SharedBudget},
    text::MaskSpan,
};
use std::sync::Arc;

#[derive(Debug)]
struct Storage {
    spans: Vec<MaskSpan>,
    account: BudgetAccount,
}
/// Sorted disjoint coverage runs. Clones share immutable storage/admission.
#[derive(Clone, Debug, Default)]
pub struct Coverage(Option<Arc<Storage>>);
impl Coverage {
    pub fn spans(&self) -> &[MaskSpan] {
        self.0.as_ref().map_or(&[], |s| &s.spans)
    }
    pub fn is_empty(&self) -> bool {
        self.spans().is_empty()
    }
    pub fn declared_bytes(&self) -> u64 {
        self.0.as_ref().map_or(0, |s| s.account.bytes())
    }
}

/// Native-size static stamps, with bilinear Q8 translations and maximum union.
/// Place the first dab immediately, then equally spaced dabs over the polyline.
/// Segment arc lengths are floored Q32 Euclidean lengths via integer isqrt;
/// interpolation rounds relative Q8 deltas with half ties away from the origin.
/// No extra endpoint dab or input-vertex dab changes a completed prefix. Color
/// opacity is applied once later by the existing `raster.mask_spans` operation.
pub fn coverage(
    tip: &SampledTip,
    points: &[(i64, i64)],
    point_scale: u32,
    spacing_q8: u32,
    clip: Rect,
    budget: &SharedBudget,
    check: &dyn Fn() -> Result<()>,
) -> Result<Coverage> {
    check()?;
    if points.is_empty()
        || points.len() > POINTS_MAX
        || !matches!(point_scale, 1 | 256)
        || spacing_q8 == 0
        || spacing_q8 > SPACING_Q8_MAX
    {
        return Err(invalid("invalid sampled stroke path, scale or spacing"));
    }
    if [clip.x0, clip.y0, clip.x1, clip.y1]
        .iter()
        .any(|v| v.unsigned_abs() > COORD_ABS_MAX as u64)
        || points.iter().any(|&(x, y)| {
            x.unsigned_abs() > (COORD_ABS_MAX * i64::from(point_scale)) as u64
                || y.unsigned_abs() > (COORD_ABS_MAX * i64::from(point_scale)) as u64
        })
    {
        return Err(invalid("sampled stroke coordinates exceed numeric policy"));
    }
    if clip.width() <= 0 || clip.height() <= 0 || tip.is_empty() {
        return Ok(Coverage::default());
    }
    let q8 = |p: (i64, i64)| {
        let factor = 256 / i64::from(point_scale);
        (p.0 * factor - clip.x0 * 256, p.1 * factor - clip.y0 * 256)
    };
    let (mut min, mut max) = ((i64::MAX, i64::MAX), (i64::MIN, i64::MIN));
    // A pixel has 2^32 arc-length units. Each segment is the floor of
    // its Euclidean length at that precision, using integer square root.
    let spacing = u128::from(spacing_q8) << 24;
    let mut length = 0u128;
    let mut previous = None;
    for (index, &point) in points.iter().enumerate() {
        if index.is_multiple_of(256) {
            check()?;
        }
        let point = q8(point);
        min = (min.0.min(point.0), min.1.min(point.1));
        max = (max.0.max(point.0), max.1.max(point.1));
        if let Some((x, y)) = previous {
            length = length
                .checked_add(segment_length((x, y), point, spacing_q8)?)
                .ok_or_else(|| limit("sampled stroke path length overflow"))?;
            if length / spacing >= DABS_MAX as u128 {
                return Err(limit("sampled stroke dab allowance exceeded"));
            }
        }
        previous = Some(point);
    }
    let anchor = tip.anchor();
    let x0 = (min.0.div_euclid(256) - i64::from(anchor.0) - 1).max(0);
    let y0 = (min.1.div_euclid(256) - i64::from(anchor.1) - 1).max(0);
    let x1 = (max.0.div_euclid(256) + i64::from(tip.width() - anchor.0) + 2).min(clip.width());
    let y1 = (max.1.div_euclid(256) + i64::from(tip.height() - anchor.1) + 2).min(clip.height());
    if x1 <= x0 || y1 <= y0 {
        return Ok(Coverage::default());
    }
    let (width, height) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let pixels = (width as u128) * (height as u128);
    if pixels > COVERAGE_PIXELS_MAX as u128 {
        return Err(limit("sampled stroke coverage allowance exceeded"));
    }
    let mut scratch = budget.account(AllocationClass::PaintMetadata);
    scratch.resize(pixels as u64)?;
    let mut mask = Vec::new();
    mask.try_reserve_exact(pixels as usize)
        .map_err(|_| limit("cannot allocate sampled stroke coverage"))?;
    scratch.resize(mask.capacity() as u64)?;
    mask.resize(pixels as usize, 0u8);
    let (mut work, mut dabs) = (0u64, 0usize);
    let mut dab = |point: (i64, i64)| -> Result<()> {
        check()?;
        dabs += 1;
        if dabs > DABS_MAX {
            return Err(limit("sampled stroke dab allowance exceeded"));
        }
        let left = point.0 - i64::from(anchor.0) * 256;
        let top = point.1 - i64::from(anchor.1) * 256;
        let a = left.div_euclid(256).max(x0);
        let b = (left.div_euclid(256) + i64::from(tip.width()) + 1).min(x1);
        let c = top.div_euclid(256).max(y0);
        let d = (top.div_euclid(256) + i64::from(tip.height()) + 1).min(y1);
        if a >= b || c >= d {
            return Ok(());
        }
        work += ((b - a) * (d - c)) as u64;
        if work > DAB_PIXELS_MAX {
            return Err(limit("sampled stroke pixel-work allowance exceeded"));
        }
        for y in c..d {
            check()?;
            for x in a..b {
                let value = translated(tip, x * 256 - left, y * 256 - top);
                let dest = &mut mask[(y - y0) as usize * width + (x - x0) as usize];
                *dest = (*dest).max(value);
            }
        }
        Ok(())
    };
    dab(q8(points[0]))?;
    let (mut travelled, mut next) = (0u128, spacing);
    for pair in points.windows(2) {
        check()?;
        let (a, b) = (q8(pair[0]), q8(pair[1]));
        let segment = segment_length(a, b, spacing_q8)?;
        if segment == 0 {
            continue;
        }
        while next <= travelled + segment {
            let numerator = next - travelled;
            dab((
                a.0 + rounded_delta(b.0 - a.0, numerator, segment),
                a.1 + rounded_delta(b.1 - a.1, numerator, segment),
            ))?;
            next += spacing;
        }
        travelled += segment;
    }
    let metadata = std::mem::size_of::<Storage>() + 2 * std::mem::size_of::<usize>();
    let mut account = budget.account(AllocationClass::PaintMetadata);
    let mut spans = Vec::new();
    for y in 0..height {
        check()?;
        let row = &mask[y * width..(y + 1) * width];
        let mut x = 0;
        while x < width {
            let value = row[x];
            let first = x;
            x += 1;
            while x < width && row[x] == value {
                x += 1;
            }
            if value == 0 {
                continue;
            }
            if spans.len() == SPANS_MAX {
                return Err(limit("sampled stroke span allowance exceeded"));
            }
            if spans.len() == spans.capacity() {
                let capacity = (spans.capacity().max(16) * 2).min(SPANS_MAX);
                account.resize((metadata + capacity * std::mem::size_of::<MaskSpan>()) as u64)?;
                spans
                    .try_reserve_exact(capacity - spans.len())
                    .map_err(|_| limit("cannot allocate sampled stroke spans"))?;
                account.resize(
                    (metadata + spans.capacity() * std::mem::size_of::<MaskSpan>()) as u64,
                )?;
            }
            spans.push(MaskSpan {
                y: clip.y0 + y0 + y as i64,
                x0: clip.x0 + x0 + first as i64,
                x1: clip.x0 + x0 + x as i64 - 1,
                coverage: value,
            });
        }
    }
    check()?;
    if spans.is_empty() {
        return Ok(Coverage::default());
    }
    Ok(Coverage(Some(Arc::new(Storage { spans, account }))))
}

// Coordinates have already passed the 47-bit pixel policy, so the Q8
// difference is below 2^57 and its squared sum fits u128. Segments long
// enough to require the maximum dab count are impossible to admit. Reject
// before the 48-bit shift: the remaining square is strictly below 2^80.
fn segment_length(a: (i64, i64), b: (i64, i64), spacing_q8: u32) -> Result<u128> {
    let dx = (i128::from(b.0) - i128::from(a.0)).unsigned_abs();
    let dy = (i128::from(b.1) - i128::from(a.1)).unsigned_abs();
    let square = dx * dx + dy * dy;
    let maximum = DABS_MAX as u128 * u128::from(spacing_q8);
    if square >= maximum * maximum {
        return Err(limit("sampled stroke segment exceeds dab allowance"));
    }
    Ok((square << 48).isqrt())
}

// Nearest Q8 translation, exact half ties away from the segment origin.
// |delta| < 2^57 and numerator <= denominator < 2^64, so multiplication
// fits u128. Add the rounded relative delta to the integer origin afterwards.
fn rounded_delta(delta: i64, numerator: u128, denominator: u128) -> i64 {
    let magnitude = (u128::from(delta.unsigned_abs()) * numerator + denominator / 2) / denominator;
    if delta < 0 {
        -(magnitude as i64)
    } else {
        magnitude as i64
    }
}

fn translated(tip: &SampledTip, x: i64, y: i64) -> u8 {
    let (ix, iy) = (x.div_euclid(256), y.div_euclid(256));
    let (fx, fy) = (x.rem_euclid(256) as u32, y.rem_euclid(256) as u32);
    let sample = |x: i64, y: i64| -> u32 {
        if x < 0 || y < 0 || x >= i64::from(tip.width()) || y >= i64::from(tip.height()) {
            0
        } else {
            u32::from(tip.alpha()[y as usize * tip.width() as usize + x as usize])
        }
    };
    ((sample(ix, iy) * (256 - fx) * (256 - fy)
        + sample(ix + 1, iy) * fx * (256 - fy)
        + sample(ix, iy + 1) * (256 - fx) * fy
        + sample(ix + 1, iy + 1) * fx * fy
        + 32768)
        / 65536) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer_arc_length_and_signed_half_ties_are_defined() {
        assert_eq!(
            segment_length((0, 0), (768, 1024), 256).unwrap(),
            1280u128 << 24
        );
        assert_eq!(
            segment_length((0, 0), (-768, -1024), 256).unwrap(),
            1280u128 << 24
        );
        assert_eq!(rounded_delta(1, 1, 2), 1);
        assert_eq!(rounded_delta(-1, 1, 2), -1);
        assert_eq!(rounded_delta(3, 1, 2), 2);
        assert_eq!(rounded_delta(-3, 1, 2), -2);
        assert_eq!(rounded_delta(3, 1, 3), 1);
        let maximum = DABS_MAX as i64;
        assert!(segment_length((0, 0), (maximum, 0), 1).is_err());
        assert!(segment_length((0, 0), (maximum - 1, 0), 1).is_ok());
        assert!(
            segment_length(
                (-COORD_ABS_MAX * 256, 0),
                (COORD_ABS_MAX * 256, 0),
                SPACING_Q8_MAX
            )
            .is_err()
        );
    }
}
