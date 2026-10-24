//! Deterministic textured brush coverage. Overlapping dabs union before blending.
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, brush::BrushMedia, text::MaskSpan};

/// Explicit temporary coverage/work limits, independent of decoded tile admission.
pub const COVERAGE_PIXELS_MAX: u128 = 4_194_304;
pub const DAB_PIXELS_MAX: u64 = 67_108_864;
const SPANS_MAX: usize = 262_144;

fn limit() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "brush coverage exceeds its work or scratch limit",
    )
}

pub fn coverage(
    points: &[(i64, i64)],
    size: u32,
    point_scale: u32,
    media: BrushMedia,
    clip: Rect,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<MaskSpan>> {
    check()?;
    if points.is_empty()
        || points.len() > 65_536
        || !(1..=1024).contains(&size)
        || !matches!(point_scale, 1 | 256)
        || media == BrushMedia::Plain
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "invalid brush medium geometry",
        ));
    }
    if clip.width() <= 0 || clip.height() <= 0 {
        return Ok(Vec::new());
    }
    let bound = paintcraft_core::types::policy::COORD_ABS_MAX;
    if points.iter().any(|&(x, y)| {
        x.unsigned_abs() > (bound * i64::from(point_scale)) as u64
            || y.unsigned_abs() > (bound * i64::from(point_scale)) as u64
    }) || [clip.x0, clip.y0, clip.x1, clip.y1]
        .iter()
        .any(|n| n.unsigned_abs() > bound as u64)
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "brush coordinates exceed numeric limits",
        ));
    }
    let scale = f64::from(point_scale);
    let radius = f64::from(size) * 0.5;
    let offset = if point_scale == 1 { 0.5 } else { 0.0 };
    let local = |p: (i64, i64)| {
        (
            (p.0 - clip.x0 * i64::from(point_scale)) as f64 / scale + offset,
            (p.1 - clip.y0 * i64::from(point_scale)) as f64 / scale + offset,
        )
    };
    let mut min = (f64::INFINITY, f64::INFINITY);
    let mut max = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for (index, &p) in points.iter().enumerate() {
        if index.is_multiple_of(256) {
            check()?;
        }
        let p = local(p);
        min = (min.0.min(p.0), min.1.min(p.1));
        max = (max.0.max(p.0), max.1.max(p.1));
    }
    let bounds = Rect::new(
        ((min.0 - radius - 1.0).floor() as i64).max(0),
        ((min.1 - radius - 1.0).floor() as i64).max(0),
        1,
        1,
    )
    .unwrap();
    let x0 = bounds.x0;
    let y0 = bounds.y0;
    let x1 = ((max.0 + radius + 1.0).ceil() as i64).min(clip.width());
    let y1 = ((max.1 + radius + 1.0).ceil() as i64).min(clip.height());
    if x1 <= x0 || y1 <= y0 {
        return Ok(Vec::new());
    }
    let width = (x1 - x0) as usize;
    let height = (y1 - y0) as usize;
    if (width as u128) * (height as u128) > COVERAGE_PIXELS_MAX {
        return Err(limit());
    }
    let mut mask = Vec::new();
    mask.try_reserve_exact(width * height)
        .map_err(|_| limit())?;
    mask.resize(width * height, 0u8);
    let mut work = 0u64;
    let spacing = (f64::from(size) / 8.0).clamp(0.5, 8.0);
    let mut dab = |cx: f64, cy: f64| -> Result<()> {
        let left = ((cx - radius - 1.0).floor() as i64).max(x0);
        let right = ((cx + radius + 1.0).ceil() as i64).min(x1);
        let top = ((cy - radius - 1.0).floor() as i64).max(y0);
        let bottom = ((cy + radius + 1.0).ceil() as i64).min(y1);
        if right <= left || bottom <= top {
            return Ok(());
        }
        work = work
            .checked_add(((right - left) * (bottom - top)) as u64)
            .ok_or_else(limit)?;
        if work > DAB_PIXELS_MAX {
            return Err(limit());
        }
        for y in top..bottom {
            check()?;
            for x in left..right {
                let alpha = tip(
                    media,
                    x as f64 + 0.5 - cx,
                    y as f64 + 0.5 - cy,
                    radius,
                    x + clip.x0,
                    y + clip.y0,
                );
                let dest = &mut mask[(y - y0) as usize * width + (x - x0) as usize];
                *dest = (*dest).max(alpha);
            }
        }
        Ok(())
    };
    dab(local(points[0]).0, local(points[0]).1)?;
    for pair in points.windows(2) {
        check()?;
        let (a, b) = (local(pair[0]), local(pair[1]));
        let Some((lo, hi)) = clipped_segment(
            a,
            b,
            radius + 1.0,
            clip.width() as f64,
            clip.height() as f64,
        ) else {
            continue;
        };
        let count = ((b.0 - a.0).hypot(b.1 - a.1) / spacing).ceil().max(1.0) as u64;
        let first = (lo * count as f64).floor() as u64;
        let last = (hi * count as f64).ceil() as u64;
        if last.saturating_sub(first) > DAB_PIXELS_MAX {
            return Err(limit());
        }
        for i in first..=last {
            check()?;
            let t = i as f64 / count as f64;
            dab(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)?;
        }
    }
    let mut spans = Vec::new();
    for (y, row) in mask.chunks_exact(width).enumerate() {
        check()?;
        let mut x = 0;
        while x < width {
            let start = x;
            let coverage = row[x];
            x += 1;
            while x < width && row[x] == coverage {
                x += 1;
            }
            if coverage != 0 {
                if spans.len() == SPANS_MAX {
                    return Err(limit());
                }
                spans.try_reserve(1).map_err(|_| limit())?;
                spans.push(MaskSpan {
                    y: y as i64 + y0 + clip.y0,
                    x0: start as i64 + x0 + clip.x0,
                    x1: x as i64 + x0 + clip.x0 - 1,
                    coverage,
                });
            }
        }
    }
    check()?;
    Ok(spans)
}

fn clipped_segment(
    a: (f64, f64),
    b: (f64, f64),
    radius: f64,
    w: f64,
    h: f64,
) -> Option<(f64, f64)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for (p, q) in [
        (-dx, a.0 + radius),
        (dx, w + radius - a.0),
        (-dy, a.1 + radius),
        (dy, h + radius - a.1),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else if p < 0.0 {
            lo = lo.max(q / p);
        } else {
            hi = hi.min(q / p);
        }
    }
    (lo <= hi).then_some((lo, hi))
}

fn tip(media: BrushMedia, dx: f64, dy: f64, radius: f64, x: i64, y: i64) -> u8 {
    let mut hash =
        (x as u64).wrapping_mul(0x9e3779b97f4a7c15) ^ (y as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    let grain = ((hash ^ (hash >> 27)) & 255) as f64 / 255.0;
    let distance = match media {
        BrushMedia::CalligraphyBrush | BrushMedia::CalligraphyPen => {
            let (u, v) = (
                (dx + dy) * std::f64::consts::FRAC_1_SQRT_2,
                (dy - dx) * std::f64::consts::FRAC_1_SQRT_2,
            );
            let (u, v) = if media == BrushMedia::CalligraphyPen {
                (v, u)
            } else {
                (u, v)
            };
            (u * u + (v / 0.24).powi(2)).sqrt()
        }
        BrushMedia::Marker => dx.abs().max(dy.abs() / 0.6),
        _ => dx.hypot(dy),
    };
    let edge = (radius + 0.5 - distance).clamp(0.0, 1.0);
    let normalized = (distance / radius).min(1.0);
    let density = match media {
        BrushMedia::Airbrush => (1.0 - normalized * normalized).powi(2) * 0.45,
        BrushMedia::Oil => 0.65 + 0.35 * ((x.rem_euclid(5) as f64) / 4.0),
        BrushMedia::Crayon => {
            if grain < 0.18 {
                0.0
            } else {
                0.35 + grain * 0.6
            }
        }
        BrushMedia::Marker => 0.45,
        BrushMedia::NaturalPencil => {
            if grain < 0.3 {
                0.0
            } else {
                grain * 0.8
            }
        }
        BrushMedia::Watercolor => (0.16 + 0.18 * grain) * (0.7 + 0.3 * normalized),
        _ => 1.0,
    };
    (edge * density * 255.0).round().clamp(0.0, 255.0) as u8
}
