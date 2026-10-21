//! Disposable sample-union state. No pixels, documents or decoded leases are retained.
use super::*;

/// One growing smooth path, keyed by its complete geometry and clipping policy.
/// A changed prefix/style/clip rebuilds; failures retire all partial derived state.
/// The byte limit bounds an explicit metadata charge, not allocator overhead/RSS.
#[derive(Debug)]
pub struct PathCache {
    limit: usize,
    state: Option<State>,
}
#[derive(Debug)]
struct State {
    size: u32,
    point_scale: u32,
    clip: Rect,
    points: Vec<(i64, i64)>,
    rows: BTreeMap<i64, Vec<(i64, i64)>>,
    charged: usize,
}
impl Default for PathCache {
    fn default() -> Self {
        Self::new(8 << 20)
    }
}
impl PathCache {
    pub fn new(metadata_charge_limit: usize) -> Self {
        Self {
            limit: metadata_charge_limit,
            state: None,
        }
    }
    pub fn clear(&mut self) {
        self.state = None;
    }
    /// Capacity-based vectors plus a conservative per-row tree/node allowance.
    /// Temporary geometry, returned spans and allocator overhead are separate.
    pub fn metadata_charge(&self) -> usize {
        self.state.as_ref().map_or(0, State::metadata_charge)
    }
    pub fn round_path_scaled(
        &mut self,
        points: &[(i64, i64)],
        size: u32,
        clip: &Rect,
        point_scale: u32,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Vec<CoverageSpan>> {
        let result = self.update(points, size, clip, point_scale, check);
        if result.is_err() || self.metadata_charge() > self.limit {
            self.clear();
        }
        result
    }
    fn update(
        &mut self,
        points: &[(i64, i64)],
        size: u32,
        clip: &Rect,
        point_scale: u32,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Vec<CoverageSpan>> {
        check()?;
        validate(points, size, clip, point_scale)?;
        if clip.x0 >= clip.x1 || clip.y0 >= clip.y1 {
            self.clear();
            return Ok(Vec::new());
        }
        let reuse = self.state.as_ref().is_some_and(|s| {
            s.size == size
                && s.point_scale == point_scale
                && s.clip == *clip
                && points.starts_with(&s.points)
        });
        if !reuse {
            self.state = Some(State {
                size,
                point_scale,
                clip: *clip,
                points: Vec::new(),
                rows: BTreeMap::new(),
                charged: 0,
            });
        }
        let state = self.state.as_mut().expect("initialized");
        let previous = state.points.len();
        let scale = i64::from(point_scale);
        let offset = if point_scale == 1 { 0.5 } else { 0.0 };
        let local = |(x, y): (i64, i64)| {
            (
                (x - clip.x0 * scale) as f64 / scale as f64 + offset,
                (y - clip.y0 * scale) as f64 / scale as f64 + offset,
            )
        };
        let local_clip = Rect {
            x0: 0,
            y0: 0,
            x1: clip.width(),
            y1: clip.height(),
        };
        if previous == 0 && points.len() == 1 {
            scan_segment(
                local(points[0]),
                local(points[0]),
                size,
                &local_clip,
                &mut state.rows,
                &mut state.charged,
                check,
            )?;
        } else if points.len() > previous {
            if previous == 1 {
                // The initial disk is contained in the first capsule. Its samples
                // can remain unioned, but the full-path work budget counts only segments.
                state.charged = 0;
            }
            for w in points[previous.saturating_sub(1)..].windows(2) {
                scan_segment(
                    local(w[0]),
                    local(w[1]),
                    size,
                    &local_clip,
                    &mut state.rows,
                    &mut state.charged,
                    check,
                )?;
            }
        }
        state
            .points
            .try_reserve(points.len() - previous)
            .map_err(|_| {
                Error::new(ErrorCode::ResourceLimit, "cannot retain smooth path points")
            })?;
        state.points.extend_from_slice(&points[previous..]);
        let mut spans = resolve_rows(&mut state.rows, check)?;
        for span in &mut spans {
            span.y += clip.y0;
            span.x0 += clip.x0;
            span.x1 += clip.x0;
        }
        check()?;
        Ok(spans)
    }
}
impl State {
    fn metadata_charge(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.points.capacity() * 16
            + self
                .rows
                .values()
                .map(|row| 128 + row.capacity() * 16)
                .sum::<usize>()
    }
}
