//! Four-by-four sample coverage of a continuous round-capped polyline.
//!
//! Subpixel spans are unioned before resolving coverage. Retracing a translucent
//! stroke therefore does not darken its joins. This is an explicit brush policy;
//! historical integer stamps remain available in `shape`.

use paintcraft_core::types::{Error, ErrorCode, Rect, Result};
use std::collections::BTreeMap;

const SCALE: i64 = 4;
/// Provisional bound on generated scan rows, including overlapping segments.
pub const SCAN_ROWS_MAX: usize = 1_048_576;

mod cache;
mod pressure;
pub use cache::PathCache;
pub use pressure::{PressurePoint, pressure_path_checked};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoverageSpan {
    pub y: i64,
    pub x0: i64,
    pub x1: i64,
    /// Number of covered samples, 1..=16.
    pub samples: u8,
}

/// Pixel centres are `(x + .5, y + .5)` for both odd and even widths.
/// Returns disjoint, sorted runs, clipped in document coordinates.
pub fn round_path(points: &[(i64, i64)], size: u32, clip: &Rect) -> Result<Vec<CoverageSpan>> {
    round_path_scaled(points, size, clip, 1)
}

/// Fixed-point input uses exact document positions in 1/256 pixel units.
/// Unlike integer input, no half-pixel center offset is added.
pub fn round_path_scaled(
    points: &[(i64, i64)],
    size: u32,
    clip: &Rect,
    point_scale: u32,
) -> Result<Vec<CoverageSpan>> {
    round_path_scaled_checked(points, size, clip, point_scale, &|| Ok(()))
}

/// As [`round_path_scaled`], with cooperative cancellation during geometry and row work.
pub fn round_path_scaled_checked(
    points: &[(i64, i64)],
    size: u32,
    clip: &Rect,
    point_scale: u32,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<CoverageSpan>> {
    check()?;
    validate(points, size, clip, point_scale)?;
    if clip.x0 >= clip.x1 || clip.y0 >= clip.y1 {
        return Ok(Vec::new());
    }
    // Rebase in integer space before converting to floating point. A translated
    // layer near the coordinate limit must retain the same subpixel coverage.
    let origin = (clip.x0, clip.y0);
    let scale = i64::from(point_scale);
    let offset = if point_scale == 1 { 0.5 } else { 0.0 };
    let local: Vec<_> = points
        .iter()
        .map(|&(x, y)| {
            (
                (x - origin.0 * scale) as f64 / scale as f64 + offset,
                (y - origin.1 * scale) as f64 / scale as f64 + offset,
            )
        })
        .collect();
    let clip = Rect {
        x0: 0,
        y0: 0,
        x1: clip.width(),
        y1: clip.height(),
    };
    check()?;
    let mut runs = rasterize(&local, size, &clip, check)?;
    for run in &mut runs {
        run.x0 += origin.0;
        run.x1 += origin.0;
        run.y += origin.1;
    }
    Ok(runs)
}

fn validate(points: &[(i64, i64)], size: u32, clip: &Rect, point_scale: u32) -> Result<()> {
    let bound = paintcraft_core::types::policy::COORD_ABS_MAX as u64;
    if !matches!(point_scale, 1 | 256)
        || !(1..=1024).contains(&size)
        || points.is_empty()
        || points.len() > 65_536
        || points.iter().any(|&(x, y)| {
            x.unsigned_abs() > bound * u64::from(point_scale)
                || y.unsigned_abs() > bound * u64::from(point_scale)
        })
        || [clip.x0, clip.x1, clip.y0, clip.y1]
            .iter()
            .any(|v| v.unsigned_abs() > bound)
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "invalid smooth brush geometry",
        ));
    }
    Ok(())
}

fn rasterize(
    points: &[(f64, f64)],
    size: u32,
    clip: &Rect,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<CoverageSpan>> {
    let mut rows = BTreeMap::new();
    let mut charged = 0;
    if points.len() == 1 {
        scan_segment(
            points[0],
            points[0],
            size,
            clip,
            &mut rows,
            &mut charged,
            check,
        )?;
    }
    for w in points.windows(2) {
        scan_segment(w[0], w[1], size, clip, &mut rows, &mut charged, check)?;
    }
    resolve_rows(&mut rows, check)
}

fn scan_segment(
    a: (f64, f64),
    b: (f64, f64),
    size: u32,
    clip: &Rect,
    rows: &mut BTreeMap<i64, Vec<(i64, i64)>>,
    charged: &mut usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    let radius = f64::from(size) / 2.0;
    scan_tapered_segment(a, b, (radius, radius), clip, rows, charged, check)
}

// Shared row scanner: the constant-radius caller retains its exact policy.
// Pressure uses the convex hull of endpoint disks: the exact linear-radius sweep.
fn scan_tapered_segment(
    a: (f64, f64),
    b: (f64, f64),
    radii: (f64, f64),
    clip: &Rect,
    rows: &mut BTreeMap<i64, Vec<(i64, i64)>>,
    charged: &mut usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    if radii == (0.0, 0.0) {
        return Ok(());
    }
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length = dx.hypot(dy);
    let (nx, ny) = if length == 0.0 {
        (0.0, 0.0)
    } else {
        (-dy / length, dx / length)
    };
    // When one disk contains the other, it contains every interpolated disk.
    // Otherwise the shared external tangents bound the exact swept envelope.
    let connecting = length > (radii.0 - radii.1).abs();
    let (positive, negative) = if radii.0 == radii.1 {
        // Retain the existing constant-radius floating-point evaluation order.
        ((nx, ny), (-nx, -ny))
    } else if connecting {
        let k = (radii.0 - radii.1) / length;
        let side = (1.0 - k * k).max(0.0).sqrt();
        let along = (dx / length * k, dy / length * k);
        (
            (along.0 + nx * side, along.1 + ny * side),
            (along.0 - nx * side, along.1 - ny * side),
        )
    } else {
        ((0.0, 0.0), (0.0, 0.0))
    };
    let quad = [
        (a.0 + positive.0 * radii.0, a.1 + positive.1 * radii.0),
        (b.0 + positive.0 * radii.1, b.1 + positive.1 * radii.1),
        (b.0 + negative.0 * radii.1, b.1 + negative.1 * radii.1),
        (a.0 + negative.0 * radii.0, a.1 + negative.1 * radii.0),
    ];
    let first = ((((a.1 - radii.0).min(b.1 - radii.1)) * 4.0).floor() as i64).max(clip.y0 * SCALE);
    let last = ((((a.1 + radii.0).max(b.1 + radii.1)) * 4.0).ceil() as i64).min(clip.y1 * SCALE);
    *charged += (last - first).max(0) as usize;
    if *charged > SCAN_ROWS_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "smooth brush scan-row budget exceeded",
        ));
    }
    for y in first..last {
        if (y - first) % 64 == 0 {
            check()?;
        }
        let sy = (y as f64 + 0.5) / 4.0;
        let (mut left, mut right) = (f64::INFINITY, f64::NEG_INFINITY);
        // The swept disk envelope is its endpoint disks plus the tangent quad.
        for (c, radius) in [(a, radii.0), (b, radii.1)] {
            if radius == 0.0 {
                continue;
            }
            let q = radius * radius - (sy - c.1).powi(2);
            if q >= 0.0 {
                let half = q.sqrt();
                left = left.min(c.0 - half);
                right = right.max(c.0 + half);
            }
        }
        if connecting {
            for i in 0..4 {
                let (p, q) = (quad[i], quad[(i + 1) % 4]);
                if p.1 != q.1 && sy >= p.1.min(q.1) && sy <= p.1.max(q.1) {
                    let x = p.0 + (sy - p.1) / (q.1 - p.1) * (q.0 - p.0);
                    left = left.min(x);
                    right = right.max(x);
                }
            }
        }
        if left <= right {
            let lo = ((left * 4.0 - 0.5).ceil() as i64).max(clip.x0 * SCALE);
            let hi = ((right * 4.0 - 0.5).floor() as i64).min(clip.x1 * SCALE - 1);
            if lo <= hi {
                rows.entry(y).or_default().push((lo, hi));
            }
        }
    }
    Ok(())
}

fn resolve_rows(
    rows: &mut BTreeMap<i64, Vec<(i64, i64)>>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Vec<CoverageSpan>> {
    let mut out = Vec::new();
    let mut events: BTreeMap<i64, i32> = BTreeMap::new();
    let mut pixel_y = None;
    for (row_index, (&y, runs)) in rows.iter_mut().enumerate() {
        if row_index % 64 == 0 {
            check()?;
        }
        let py = y.div_euclid(SCALE);
        if pixel_y != Some(py) {
            if let Some(prev) = pixel_y {
                resolve(prev, &events, &mut out);
            }
            events.clear();
            pixel_y = Some(py);
        }
        runs.sort_unstable();
        let mut merged: Option<(i64, i64)> = None;
        let mut retained = 0;
        for index in 0..runs.len() {
            let (lo, hi) = runs[index];
            if let Some((a, b)) = merged {
                if lo <= b + 1 {
                    merged = Some((a, b.max(hi)));
                } else {
                    add_samples(a, b, &mut events);
                    runs[retained] = (a, b);
                    retained += 1;
                    merged = Some((lo, hi));
                }
            } else {
                merged = Some((lo, hi));
            }
        }
        if let Some((a, b)) = merged {
            add_samples(a, b, &mut events);
            runs[retained] = (a, b);
            retained += 1;
        }
        runs.truncate(retained);
    }
    if let Some(y) = pixel_y {
        resolve(y, &events, &mut out);
    }
    check()?;
    Ok(out)
}

fn add_samples(lo: i64, hi: i64, events: &mut BTreeMap<i64, i32>) {
    let (first, last) = (lo.div_euclid(SCALE), hi.div_euclid(SCALE));
    let mut add = |a, b, n| {
        *events.entry(a).or_default() += n;
        *events.entry(b + 1).or_default() -= n;
    };
    if first == last {
        add(first, last, (hi - lo + 1) as i32);
    } else {
        add(first, first, (SCALE - lo.rem_euclid(SCALE)) as i32);
        if first + 1 < last {
            add(first + 1, last - 1, 4);
        }
        add(last, last, (hi.rem_euclid(SCALE) + 1) as i32);
    }
}

fn resolve(y: i64, events: &BTreeMap<i64, i32>, out: &mut Vec<CoverageSpan>) {
    let (mut count, mut previous) = (0, None);
    for (&x, &delta) in events {
        if let Some(start) = previous
            && count > 0
            && start < x
        {
            debug_assert!(count <= 16);
            out.push(CoverageSpan {
                y,
                x0: start,
                x1: x - 1,
                samples: count as u8,
            });
        }
        count += delta;
        previous = Some(x);
    }
}
