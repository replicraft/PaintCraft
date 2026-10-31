//! Source5 native stepping with one retained generic prepared-program owner.
use super::*;
use paintcraft_core::types::{
    Rect,
    resources::{BudgetAccount, BudgetScope},
};
use paintcraft_render::{
    display::DisplayAccumulator,
    spatial::{WorkspaceAdmission, with_workspace},
};
use protocol::programs::Profile;
use protocol::spatial::{NATIVE_EDGE, NATIVE_STEP_WORK_MAX, PATCH_EDGE};

fn patch(request: DisplayRequest, column: u32, row: u32) -> Rect {
    let step = i64::from(request.step());
    let region = request.region();
    Rect {
        x0: region.x0 + i64::from(column) * step,
        y0: region.y0 + i64::from(row) * step,
        x1: (region.x0 + i64::from(column + PATCH_EDGE) * step).min(region.x1),
        y1: (region.y0 + i64::from(row + PATCH_EDGE) * step).min(region.y1),
    }
}
fn each_tile(
    request: DisplayRequest,
    mut visit: impl FnMut(Rect, u64) -> Result<()>,
) -> Result<()> {
    let [width, height] = request.size();
    let mut row = 0;
    while row < height {
        let mut column = 0;
        while column < width {
            let area = patch(request, column, row);
            let mut y = area.y0;
            while y < area.y1 {
                let mut x = area.x0;
                while x < area.x1 {
                    let rect = Rect {
                        x0: x,
                        y0: y,
                        x1: (x + NATIVE_EDGE).min(area.x1),
                        y1: (y + NATIVE_EDGE).min(area.y1),
                    };
                    let resolve = if rect.x1 == area.x1 && rect.y1 == area.y1 {
                        ((area.width() + i64::from(request.step()) - 1) / i64::from(request.step())
                            * ((area.height() + i64::from(request.step()) - 1)
                                / i64::from(request.step()))) as u64
                    } else {
                        0
                    };
                    visit(rect, resolve)?;
                    x = rect.x1;
                }
                y = (y + NATIVE_EDGE).min(area.y1);
            }
            column += PATCH_EDGE;
        }
        row += PATCH_EDGE;
    }
    Ok(())
}
#[derive(Clone, Copy, Debug)]
struct Quote {
    spatial: u64,
    other: u64,
    peak: u64,
    tiles: u64,
}
fn quote_tile(doc: &Document, tile: Rect, resolve: u64, catalog: bool) -> Result<Quote> {
    let mut result = Quote {
        spatial: 0,
        other: (tile.area() as u64)
            .checked_add(resolve)
            .ok_or_else(|| resource("Native tile work overflows"))?,
        peak: 0,
        tiles: 1,
    };
    for layer in doc
        .layers_in_order()
        .filter(|layer| layer.visible && layer.opacity != 0)
    {
        let Some(clipped) = tile.intersect(&layer.extent()) else {
            continue;
        };
        let local = clipped
            .translated(-layer.origin.0, -layer.origin.1)
            .ok_or_else(|| bad("Layer view coordinates"))?;
        let plan = paintcraft_render::spatial::quote_layer(layer, local, &|| Ok(()))?;
        result.peak = result.peak.max(plan.bytes);
        result.other = result
            .other
            .checked_add(clipped.area() as u64)
            .ok_or_else(|| resource("Composite work overflows"))?;
        let spatial = paintcraft_render::spatial::active(layer);
        // Source4 prices both the uniform proof and possible dense fallback.
        // Spatial workspaces evaluate each stage once; source3 stays unchanged.
        let layer_work = if catalog && !spatial {
            plan.work
                .checked_mul(2)
                .ok_or_else(|| resource("Point proof work overflows"))?
        } else {
            plan.work
        };
        let target = if spatial {
            &mut result.spatial
        } else {
            &mut result.other
        };
        *target = target
            .checked_add(layer_work)
            .ok_or_else(|| resource("Layer work overflows"))?;
    }
    if result
        .other
        .checked_add(result.spatial)
        .is_none_or(|n| n > NATIVE_STEP_WORK_MAX)
    {
        return Err(resource("Native tile exceeds single-step work profile"));
    }
    Ok(result)
}
fn quote(doc: &Document, request: DisplayRequest, profile: Profile) -> Result<Quote> {
    let area = request.region();
    let native = u64::try_from(area.area()).map_err(|_| resource("Native view work overflows"))?;
    if native > profile.work {
        return Err(resource("Spatial native view exceeds job work"));
    }
    let side = i64::from(request.step());
    if u64::try_from(side.min(area.width()) * side.min(area.height())).unwrap_or(u64::MAX)
        > paintcraft_render::RENDER_PIXELS_MAX
    {
        return Err(resource("Spatial display cell exceeds accumulator bound"));
    }
    let mut total = Quote {
        spatial: 0,
        other: 0,
        peak: 0,
        tiles: 0,
    };
    each_tile(request, |tile, resolve| {
        let next = quote_tile(doc, tile, resolve, true)?;
        total.peak = total.peak.max(next.peak);
        total.spatial = total
            .spatial
            .checked_add(next.spatial)
            .ok_or_else(|| resource("Spatial job work overflow"))?;
        total.other = total
            .other
            .checked_add(next.other)
            .ok_or_else(|| resource("Composite job work overflow"))?;
        total.tiles = total
            .tiles
            .checked_add(1)
            .ok_or_else(|| resource("Native tile count overflow"))?;
        if total.peak > profile.bytes
            || total
                .other
                .checked_add(total.spatial)
                .is_none_or(|n| n > profile.work)
        {
            return Err(resource("Spatial frame exceeds declared workspace/work"));
        }
        Ok(())
    })?;
    Ok(total)
}
#[derive(Debug)]
pub(super) struct GenericFrame {
    sums: Vec<DisplayAccumulator>,
    native: Option<(i64, i64)>,
    admission: WorkspaceAdmission,
    other_used: u64,
    other_limit: u64,
    _fixed: BudgetAccount,
    #[cfg(test)]
    pub tiles: u64,
}
impl GenericFrame {
    /// No pixel rendering here. Quote every exact native tile before accepting
    /// the frame, including repeated prefix halos and nonspatial composition.
    pub fn prepare(
        doc: &Document,
        request: DisplayRequest,
        profile: Profile,
        scope: BudgetScope,
        render: bool,
    ) -> Result<Option<Self>> {
        let quote = quote(doc, request, profile)?;
        let admission = paintcraft_render::spatial::capture_workspace_or_current(
            || WorkspaceAdmission::new(scope, profile.bytes, profile.work),
            profile.bytes,
            profile.work,
        )?;
        let bound = admission.has_prepared_programs();
        let programs = paintcraft_render::prepared::quote_document_programs(doc, &|| Ok(()))?;
        // An existing bound job is never extended. Validate even quote-only
        // requests, before cache use; this helper cannot compile a missing key.
        let admission = if bound {
            paintcraft_render::prepared::prepare_document_programs(doc, admission, &|| Ok(()))?
        } else {
            admission
        };
        let retained = if bound {
            admission.prepared_retained_bytes()
        } else {
            programs.retained_bytes
        };
        let compile = if bound { 0 } else { programs.compile_work };
        let prep = if bound {
            0
        } else {
            programs.preparation_peak_bytes
        };
        let fixed_bytes = 1024 * std::mem::size_of::<DisplayAccumulator>() as u64 + (64 * 64 * 4);
        let peak = retained
            .checked_add(quote.peak)
            .and_then(|n| n.checked_add(fixed_bytes))
            .map(|n| n.max(prep))
            .ok_or_else(|| resource("Prepared frame peak overflow"))?;
        let work = quote
            .other
            .checked_add(quote.spatial)
            .and_then(|n| n.checked_add(compile))
            .and_then(|n| n.checked_add(admission.work_used()))
            .ok_or_else(|| resource("Prepared frame work overflow"))?;
        if peak > admission.bytes_limit() || work > admission.work_limit() {
            return Err(resource("Prepared frame exceeds caller workspace/work"));
        }
        if !render {
            return Ok(None);
        }
        admission.reserve_work(quote.other)?;
        let admission =
            paintcraft_render::prepared::prepare_document_programs(doc, admission, &|| Ok(()))?;
        let mut fixed = admission.scope().account();
        fixed.resize(fixed_bytes)?;
        let mut sums = Vec::new();
        sums.try_reserve_exact(1024)
            .map_err(|_| resource("Prepared accumulator allocation"))?;
        let actual_fixed = sums.capacity() as u64
            * std::mem::size_of::<DisplayAccumulator>() as u64
            + (64 * 64 * 4);
        let actual_peak = admission
            .prepared_retained_bytes()
            .checked_add(quote.peak)
            .and_then(|n| n.checked_add(actual_fixed))
            .ok_or_else(|| resource("Prepared actual frame peak"))?;
        if actual_peak > admission.bytes_limit() {
            return Err(resource("Prepared actual capacity exceeds caller"));
        }
        fixed.resize(actual_fixed)?;
        sums.resize(1024, DisplayAccumulator::default());
        Ok(Some(Self {
            sums,
            native: None,
            admission,
            other_used: 0,
            other_limit: quote.other,
            _fixed: fixed,
            #[cfg(test)]
            tiles: quote.tiles,
        }))
    }
    pub fn step(&mut self, doc: &Document, frame: &mut Frame) -> Result<bool> {
        let [width, height] = frame.request.size();
        if frame.row >= height {
            return Ok(true);
        }
        let area = patch(frame.request, frame.column, frame.row);
        let (x, y) = self.native.unwrap_or_else(|| {
            self.sums.fill(DisplayAccumulator::default());
            (area.x0, area.y0)
        });
        let tile = Rect {
            x0: x,
            y0: y,
            x1: (x + NATIVE_EDGE).min(area.x1),
            y1: (y + NATIVE_EDGE).min(area.y1),
        };
        let step = i64::from(frame.request.step());
        let resolve = if tile.x1 == area.x1 && tile.y1 == area.y1 {
            ((area.width() + step - 1) / step * ((area.height() + step - 1) / step)) as u64
        } else {
            0
        };
        let other = quote_tile(doc, tile, resolve, true)?.other;
        self.other_used = self
            .other_used
            .checked_add(other)
            .filter(|n| *n <= self.other_limit)
            .ok_or_else(|| resource("Native composition work exceeded its preflight"))?;
        let pixels = with_workspace(self.admission.clone(), || {
            paintcraft_render::render_region_cancellable(doc, tile, || Ok(()))
        })?;
        if pixels.capacity() > 64 * 64 * 4 {
            return Err(resource("Prepared native tile capacity exceeds fixed slot"));
        }
        let step = i64::from(frame.request.step());
        for (row, pixels) in pixels.chunks_exact(tile.width() as usize * 4).enumerate() {
            let sy = (tile.y0 + row as i64 - area.y0) / step;
            for (column, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
                let sx = (tile.x0 + column as i64 - area.x0) / step;
                self.sums[sy as usize * PATCH_EDGE as usize + sx as usize]
                    .add(paintcraft_core::types::Rgba(*pixel))?;
            }
        }
        let next_x = tile.x1;
        let (next_x, next_y) = if next_x >= area.x1 {
            (area.x0, tile.y1)
        } else {
            (next_x, y)
        };
        if next_y < area.y1 {
            self.native = Some((next_x, next_y));
            return Ok(false);
        }
        let w = ((area.width() + step - 1) / step) as usize;
        let h = ((area.height() + step - 1) / step) as usize;
        for y in 0..h {
            for x in 0..w {
                let sum = &self.sums[y * PATCH_EDGE as usize + x];
                let expected = (step.min(area.width() - x as i64 * step)
                    * step.min(area.height() - y as i64 * step))
                    as u64;
                if sum.samples() != expected {
                    return Err(bad("Spatial display cell sample count changed"));
                }
                let dst =
                    ((frame.row as usize + y) * width as usize + frame.column as usize + x) * 4;
                frame.pixels[dst..dst + 4].copy_from_slice(&sum.resolve()?.0);
            }
        }
        self.native = None;
        frame.column += PATCH_EDGE;
        if frame.column >= width {
            frame.column = 0;
            frame.row += PATCH_EDGE;
        }
        Ok(frame.row >= height)
    }
    #[cfg(test)]
    pub fn tiles(&self) -> u64 {
        self.tiles
    }
    #[cfg(test)]
    pub fn work_used(&self) -> u64 {
        self.admission.work_used()
    }
}
