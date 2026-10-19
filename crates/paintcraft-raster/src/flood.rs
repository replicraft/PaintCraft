//! 4-connected scanline flood fill on layer-local pixels.
//!
//! The region is every pixel reachable from the seed whose four channels each
//! differ from the seed pixel's by at most `tolerance`. The region is computed
//! entirely on the original pixels before anything is painted, so painting
//! cannot change what counts as inside.

use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba};

use crate::raster::RasterLayer;
use crate::shape::Span;

/// Largest area a fill may examine (bounds its visited bitmap).
pub const FLOOD_AREA_MAX: u128 = 67_108_864;

fn near(a: Rgba, b: Rgba, tol: u8) -> bool {
    a.0.iter()
        .zip(b.0.iter())
        .all(|(x, y)| x.abs_diff(*y) <= tol)
}

/// Spans, in layer-local coordinates, of the region around `seed` inside `clip`.
/// A seed outside the clip yields no region.
pub fn region(
    layer: &RasterLayer,
    seed: (i64, i64),
    clip: &Rect,
    tolerance: u8,
) -> Result<Vec<Span>> {
    if !clip.contains(seed.0, seed.1) {
        return Ok(Vec::new());
    }
    if clip.area() > FLOOD_AREA_MAX {
        return Err(Error::new(
            ErrorCode::ResourceLimit,
            "fill area exceeds the flood-fill limit",
        ));
    }
    let (w, h) = (clip.width() as usize, clip.height() as usize);
    let mut seen = vec![0u64; (w * h).div_ceil(64)];
    let idx = |x: i64, y: i64| (y - clip.y0) as usize * w + (x - clip.x0) as usize;
    let is_seen = |seen: &[u64], x, y| seen[idx(x, y) / 64] >> (idx(x, y) % 64) & 1 == 1;
    let target = layer.get(seed.0, seed.1);
    let matches = |x: i64, y: i64| near(layer.get(x, y), target, tolerance);

    let mut out = Vec::new();
    let mut stack = vec![seed];
    while let Some((x, y)) = stack.pop() {
        if is_seen(&seen, x, y) || !matches(x, y) {
            continue;
        }
        let (mut l, mut r) = (x, x);
        while l > clip.x0 && !is_seen(&seen, l - 1, y) && matches(l - 1, y) {
            l -= 1;
        }
        while r < clip.x1 - 1 && !is_seen(&seen, r + 1, y) && matches(r + 1, y) {
            r += 1;
        }
        for xi in l..=r {
            let i = idx(xi, y);
            seen[i / 64] |= 1 << (i % 64);
        }
        out.push(Span { y, x0: l, x1: r });
        for ny in [y - 1, y + 1] {
            if !(clip.y0..clip.y1).contains(&ny) {
                continue;
            }
            let mut inside_run = false;
            for xi in l..=r {
                let ok = !is_seen(&seen, xi, ny) && matches(xi, ny);
                if ok && !inside_run {
                    stack.push((xi, ny));
                }
                inside_run = ok;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::{Admission, BlendMode};

    const K: Rgba = Rgba([0, 0, 0, 255]);

    fn layer(rows: &[&str]) -> RasterLayer {
        let mut l = RasterLayer::new(rows[0].len() as u32, rows.len() as u32, Rgba::TRANSPARENT);
        let mut adm = Admission::new(1 << 24);
        for (y, row) in rows.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                if c == '#' {
                    l.blend_pixel(x as i64, y as i64, K, BlendMode::Replace, &mut adm)
                        .unwrap();
                }
            }
        }
        l
    }

    fn cells(spans: &[Span]) -> usize {
        spans.iter().map(|s| (s.x1 - s.x0 + 1) as usize).sum()
    }

    #[test]
    fn fills_only_the_connected_region_and_not_diagonals() {
        let l = layer(&["..#..", "..#..", "###..", "....#", "....."]);
        let clip = l.bounds();
        // Top-left pocket: (0,0),(1,0),(0,1),(1,1) — walled by the hash cells.
        assert_eq!(cells(&region(&l, (0, 0), &clip, 0).unwrap()), 4);
        // Everything else transparent is one region through the open bottom/right.
        let rest = region(&l, (4, 0), &clip, 0).unwrap();
        assert_eq!(cells(&rest), 25 - 4 - 6);
        // The lone hash cell at (4,3) is not joined to the wall diagonally.
        assert_eq!(cells(&region(&l, (4, 3), &clip, 0).unwrap()), 1);
    }

    #[test]
    fn tolerance_is_per_channel_and_seed_relative() {
        let mut l = RasterLayer::new(4, 1, Rgba([100, 100, 100, 255]));
        let mut adm = Admission::new(1 << 24);
        for (x, v) in [(1, 105u8), (2, 111), (3, 100)] {
            l.blend_pixel(x, 0, Rgba([v, 100, 100, 255]), BlendMode::Replace, &mut adm)
                .unwrap();
        }
        let n = |tol| cells(&region(&l, (0, 0), &l.bounds(), tol).unwrap());
        assert_eq!((n(0), n(5), n(10), n(11)), (1, 2, 2, 4));
    }

    #[test]
    fn the_clip_bounds_the_region_and_a_seed_outside_is_nothing() {
        let l = layer(&["....", "....", "...."]);
        let clip = Rect::new(1, 1, 2, 1).unwrap();
        assert_eq!(cells(&region(&l, (1, 1), &clip, 0).unwrap()), 2);
        assert!(region(&l, (0, 0), &clip, 0).unwrap().is_empty());
    }

    #[test]
    fn oversized_areas_are_refused_and_spiral_mazes_terminate() {
        let l = RasterLayer::new(1_048_576, 1_048_576, Rgba::TRANSPARENT);
        assert_eq!(
            region(&l, (0, 0), &l.bounds(), 0).unwrap_err().code,
            ErrorCode::ResourceLimit
        );
        let rows = [
            "#########",
            "#.......#",
            "#.#####.#",
            "#.#...#.#",
            "#.#.#.#.#",
            "#.#.###.#",
            "#.#.....#",
            "#.#######",
            "#########",
        ];
        let maze = layer(&rows);
        let total = region(&maze, (1, 1), &maze.bounds(), 255).unwrap();
        assert_eq!(cells(&total), 81, "tolerance 255 matches everything");
        let corridor = region(&maze, (0, 0), &maze.bounds(), 0).unwrap();
        assert_eq!(
            cells(&corridor),
            rows.iter()
                .flat_map(|r| r.chars())
                .filter(|c| *c == '#')
                .count()
        );
    }
}
