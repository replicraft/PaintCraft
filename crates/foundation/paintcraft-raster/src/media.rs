//! Deterministic textured brush coverage. Overlapping dabs union before blending.
use paintcraft_core::types::{
    Error, ErrorCode, Rect, Result,
    brush::BrushMedia,
    resources::{AllocationClass, SharedBudget},
    text::MaskSpan,
};
pub mod dynamics;
use dynamics::{Attributes, Point};

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
    if media == BrushMedia::Plain {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "invalid brush medium geometry",
        ));
    }
    let mut spans = Vec::new();
    visit(
        points.len(),
        &|i| Point {
            at: points[i],
            attributes: Attributes::default(),
        },
        size,
        point_scale,
        media,
        clip,
        None,
        check,
        &mut |span| {
            spans.try_reserve(1).map_err(|_| limit())?;
            spans.push(span);
            Ok(())
        },
    )?;
    Ok(spans)
}

#[allow(clippy::too_many_arguments)]
fn visit(
    count: usize,
    point: &dyn Fn(usize) -> Point,
    size: u32,
    point_scale: u32,
    media: BrushMedia,
    clip: Rect,
    budget: Option<&SharedBudget>,
    check: &dyn Fn() -> Result<()>,
    emit: &mut dyn FnMut(MaskSpan) -> Result<()>,
) -> Result<()> {
    check()?;
    if count == 0
        || count > 65_536
        || !(1..=1024).contains(&size)
        || !matches!(point_scale, 1 | 256)
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "invalid brush medium geometry",
        ));
    }
    if clip.width() <= 0 || clip.height() <= 0 {
        return Ok(());
    }
    let bound = paintcraft_core::types::policy::COORD_ABS_MAX;
    if (0..count).any(|i| {
        let Point {
            at: (x, y),
            attributes,
        } = point(i);
        x.unsigned_abs() > (bound * i64::from(point_scale)) as u64
            || y.unsigned_abs() > (bound * i64::from(point_scale)) as u64
            || !attributes.valid()
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
    for index in 0..count {
        if index.is_multiple_of(256) {
            check()?;
        }
        let p = local(point(index).at);
        min = (min.0.min(p.0), min.1.min(p.1));
        max = (max.0.max(p.0), max.1.max(p.1));
    }
    let extent = if media == BrushMedia::Marker
        && (0..count).any(|i| point(i).attributes.rotation_q8 != 0)
    {
        radius * std::f64::consts::SQRT_2
    } else {
        radius
    };
    let bounds = Rect::new(
        ((min.0 - extent - 1.0).floor() as i64).max(0),
        ((min.1 - extent - 1.0).floor() as i64).max(0),
        1,
        1,
    )
    .unwrap();
    let x0 = bounds.x0;
    let y0 = bounds.y0;
    let x1 = ((max.0 + extent + 1.0).ceil() as i64).min(clip.width());
    let y1 = ((max.1 + extent + 1.0).ceil() as i64).min(clip.height());
    if x1 <= x0 || y1 <= y0 {
        return Ok(());
    }
    let width = (x1 - x0) as usize;
    let height = (y1 - y0) as usize;
    if (width as u128) * (height as u128) > COVERAGE_PIXELS_MAX {
        return Err(limit());
    }
    let mut scratch = budget.map(|b| b.account(AllocationClass::PaintMetadata));
    if let Some(account) = &mut scratch {
        account.resize((width * height) as u64)?;
    }
    let mut mask = Vec::new();
    mask.try_reserve_exact(width * height)
        .map_err(|_| limit())?;
    if let Some(account) = &mut scratch {
        account.resize(mask.capacity() as u64)?;
    }
    mask.resize(width * height, 0u8);
    let mut work = 0u64;
    let mut attempts = 0usize;
    // Thin nibs need closer dabs so their exposed edge does not scallop.
    let spacing = match media {
        BrushMedia::CalligraphyPen => (f64::from(size) / 32.0).clamp(0.25, 4.0),
        BrushMedia::CalligraphyBrush => (f64::from(size) / 20.0).clamp(0.35, 6.0),
        _ => (f64::from(size) / 8.0).clamp(0.5, 8.0),
    };
    let mut dab = |cx: f64,
                   cy: f64,
                   tangent: (f64, f64),
                   travelled: f64,
                   attributes: Attributes|
     -> Result<()> {
        // Attributed zero-width/opacity dabs still cost traversal work. Keep
        // the historical API's admission unchanged; this is the new profile's
        // global attempt bound, independent of the pixel-work allowance.
        if budget.is_some() {
            attempts += 1;
            if attempts > dynamics::DABS_MAX {
                return Err(limit());
            }
        }
        let radius = radius * f64::from(attributes.width) / 65535.0;
        if radius == 0.0 || attributes.opacity == 0 {
            return Ok(());
        }
        // Oil loses its load gradually along the gesture. Earlier coverage stays
        // fixed, so a growing preview and the released stroke remain identical.
        let load = if media == BrushMedia::Oil {
            (1.0 - (travelled - f64::from(size) * 2.0).max(0.0) / (f64::from(size) * 18.0))
                .clamp(0.0, 1.0)
        } else {
            1.0
        };
        if load == 0.0 {
            return Ok(());
        }
        let tip_radius = if media == BrushMedia::Oil {
            radius * (0.30 + 0.70 * load)
        } else {
            radius
        };
        let extent = if media == BrushMedia::Marker && attributes.rotation_q8 != 0 {
            radius * std::f64::consts::SQRT_2
        } else {
            radius
        };
        let rotation = attributes.rotation();
        let left = ((cx - extent - 1.0).floor() as i64).max(x0);
        let right = ((cx + extent + 1.0).ceil() as i64).min(x1);
        let top = ((cy - extent - 1.0).floor() as i64).max(y0);
        let bottom = ((cy + extent + 1.0).ceil() as i64).min(y1);
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
                let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                let (dx, dy) = match rotation {
                    Some((sin, cos)) => (dx * cos + dy * sin, dy * cos - dx * sin),
                    None => (dx, dy),
                };
                let alpha = tip(
                    media,
                    dx,
                    dy,
                    tip_radius,
                    (x + clip.x0, y + clip.y0),
                    tangent,
                );
                let dest = &mut mask[(y - y0) as usize * width + (x - x0) as usize];
                let loaded = (f64::from(alpha) * load).round() as u8;
                let value =
                    ((u32::from(loaded) * u32::from(attributes.opacity) + 32767) / 65535) as u8;
                *dest = (*dest).max(value);
            }
        }
        Ok(())
    };
    let direction = |a: (f64, f64), b: (f64, f64)| {
        let length = (b.0 - a.0).hypot(b.1 - a.1);
        if length > 0.0 {
            ((b.0 - a.0) / length, (b.1 - a.1) / length)
        } else {
            (1.0, 0.0)
        }
    };
    let start = local(point(0).at);
    let tangent = if count > 1 {
        direction(start, local(point(1).at))
    } else {
        (1.0, 0.0)
    };
    dab(start.0, start.1, tangent, 0.0, point(0).attributes)?;
    let mut travelled = 0.0;
    for index in 1..count {
        check()?;
        if media == BrushMedia::Oil && travelled >= f64::from(size) * 20.0 {
            break;
        }
        let (before, after) = (point(index - 1), point(index));
        let (a, b) = (local(before.at), local(after.at));
        let length = (b.0 - a.0).hypot(b.1 - a.1);
        let Some((lo, hi)) = clipped_segment(
            a,
            b,
            extent + 1.0,
            clip.width() as f64,
            clip.height() as f64,
        ) else {
            travelled += length;
            continue;
        };
        let hi = if media == BrushMedia::Oil && length > 0.0 {
            hi.min(((f64::from(size) * 20.0 - travelled) / length).max(0.0))
        } else {
            hi
        };
        if hi < lo {
            travelled += length;
            continue;
        }
        let segment_spacing = if budget.is_some() {
            dynamics::spacing(size, media, before.attributes, after.attributes)
        } else {
            spacing
        };
        let count = (length / segment_spacing).ceil().max(1.0) as u64;
        let first = (lo * count as f64).floor() as u64;
        let last = (hi * count as f64).ceil() as u64;
        if last.saturating_sub(first) > DAB_PIXELS_MAX {
            return Err(limit());
        }
        for i in first..=last {
            check()?;
            let t = i as f64 / count as f64;
            dab(
                a.0 + (b.0 - a.0) * t,
                a.1 + (b.1 - a.1) * t,
                direction(a, b),
                travelled + length * t,
                before.attributes.interpolate(after.attributes, t),
            )?;
        }
        travelled += length;
    }
    let mut spans = 0usize;
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
                if spans == SPANS_MAX {
                    return Err(limit());
                }
                spans += 1;
                emit(MaskSpan {
                    y: y as i64 + y0 + clip.y0,
                    x0: start as i64 + x0 + clip.x0,
                    x1: x as i64 + x0 + clip.x0 - 1,
                    coverage,
                })?;
            }
        }
    }
    check()?;
    Ok(())
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

fn tip(
    media: BrushMedia,
    dx: f64,
    dy: f64,
    radius: f64,
    position: (i64, i64),
    tangent: (f64, f64),
) -> u8 {
    let (x, y) = position;
    let grain = noise(x, y);
    let (distance, edge_scale) = match media {
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
            let narrow = if media == BrushMedia::CalligraphyPen {
                0.18
            } else {
                0.30
            };
            // A broad, angled nib for the brush and a finer opposing pen nib.
            let distance = u.hypot(v / narrow);
            // Convert elliptical distance to screen-space distance near the nib
            // boundary, keeping its antialias fringe a full pixel wide.
            let gradient = if distance > 0.0 {
                u.hypot(v / (narrow * narrow)) / distance
            } else {
                1.0
            };
            (distance, gradient)
        }
        BrushMedia::Marker => {
            // A broad felt tip with softened corners rather than a circular pen.
            let qx = (dx.abs() - radius * 0.82).max(0.0);
            let qy = (dy.abs() - radius * 0.42).max(0.0);
            (qx.hypot(qy) + radius * 0.82, 1.0)
        }
        _ => (dx.hypot(dy), 1.0),
    };
    let edge = (0.5 + (radius - distance) / edge_scale).clamp(0.0, 1.0);
    let normalized = (distance / radius).min(1.0);
    let density = match media {
        BrushMedia::Airbrush => {
            // Discrete spray particles, rather than a uniformly translucent disc.
            if grain < (1.0 - normalized * normalized).powi(2) * 0.60 {
                1.0
            } else {
                0.0
            }
        }
        BrushMedia::Oil => {
            // An opaque loaded center, with longitudinal bristle grooves only at
            // the exposed edge. Groove direction follows the stroke, not world X.
            let across = -dx * tangent.1 + dy * tangent.0;
            let bristle = ((across * 2.4).cos() * 0.5 + 0.5).powi(2);
            1.0 - normalized.powi(6) * (0.12 + 0.55 * bristle)
        }
        BrushMedia::Crayon => {
            // Broad wax catches the paper ridges, leaving clustered dry flakes
            // and smaller pits throughout the mark instead of a solid fill.
            let paper = paper_noise(x, y, 3);
            if paper < 0.32 || (paper < 0.58 && grain < 0.22) {
                0.0
            } else {
                0.80 + 0.20 * paper
            }
        }
        BrushMedia::Marker => 1.0,
        BrushMedia::NaturalPencil => {
            // Fine graphite with both grain and pressure variation across paper.
            let paper = paper_noise(x, y, 2);
            if grain < 0.16 {
                0.0
            } else {
                (0.30 + 0.45 * grain) * (0.70 + 0.30 * paper)
            }
        }
        BrushMedia::Watercolor => {
            // A wet, softly feathered wash. Correlated pigment variation avoids
            // the previous spray-like noise. No distance-based paint depletion.
            let pigment = paper_noise(x, y, 8);
            (0.56 + 0.16 * pigment) * (1.0 - 0.60 * normalized.powi(4))
        }
        _ => 1.0,
    };
    (edge * density * 255.0).round().clamp(0.0, 255.0) as u8
}

// Stable paper-space texture keeps preview, final publication and retracing equal.
fn noise(x: i64, y: i64) -> f64 {
    let mut hash =
        (x as u64).wrapping_mul(0x9e3779b97f4a7c15) ^ (y as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    ((hash ^ (hash >> 27)) & 255) as f64 / 255.0
}
fn paper_noise(x: i64, y: i64, scale: i64) -> f64 {
    let (cx, cy) = (x.div_euclid(scale), y.div_euclid(scale));
    let smooth = |value: i64| {
        let t = value.rem_euclid(scale) as f64 / scale as f64;
        t * t * (3.0 - 2.0 * t)
    };
    let (tx, ty) = (smooth(x), smooth(y));
    let top = noise(cx, cy) * (1.0 - tx) + noise(cx + 1, cy) * tx;
    let bottom = noise(cx, cy + 1) * (1.0 - tx) + noise(cx + 1, cy + 1) * tx;
    top * (1.0 - ty) + bottom * ty
}
