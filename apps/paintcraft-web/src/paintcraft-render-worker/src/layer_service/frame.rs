use super::*;
use paintcraft_core::types::Rect;
use paintcraft_render::display::DisplayAccumulator;
use quote::{FrameQuote, PATCH};
#[derive(Debug)]
pub(super) struct AppearanceFrame {
    pub pixels: Vec<u8>,
    sums: Vec<DisplayAccumulator>,
    pub binding: Value,
    request: DisplayRequest,
    pub job: OpenJob,
    native: Option<(i64, i64)>,
    column: u32,
    row: u32,
    policy: SlicePolicy,
    other_used: u64,
    other_limit: u64,
    _scratch: BudgetAccount,
    pub output: BudgetAccount,
    pub delivered: bool,
    pub(super) remaining: WorkComponents,
    pub(super) total_work: WorkComponents,
    pub(super) mask_actual: u64,
    pub(super) mask_upper: u64,
    pub(super) copy: Option<super::copy::Reservation>,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(super) encoded_account: Option<BudgetAccount>,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct PreparedQuote {
    pub frame: FrameQuote,
    pub work: WorkComponents,
    pub peak: u64,
    pub fixed: u64,
}
impl AppearanceFrame {
    pub fn quote(
        doc: &Document,
        request: DisplayRequest,
        job: &OpenJob,
        render: bool,
        policy: SlicePolicy,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<PreparedQuote> {
        let metadata_check = || {
            job.spend_control(control::CHECK_SLOTS * if render { 2 } else { 1 })?;
            check()
        };
        job.validate_document(doc, &metadata_check)?;
        let frame = quote::frame_quote(doc, request, policy, job, render, check)?;
        let programs = paintcraft_render::prepared::quote_document_programs(doc, &metadata_check)?;
        let compiled = job.workspace().has_prepared_programs();
        let compile = if compiled { 0 } else { programs.compile_work };
        let retained = if compiled { 0 } else { programs.retained_bytes };
        let preparation = if compiled {
            0
        } else {
            programs.preparation_peak_bytes
        };
        let existing = job.workspace().scope().footprint().live;
        let fixed = (PATCH as u64)
            .checked_mul(PATCH as u64)
            .and_then(|n| n.checked_mul(std::mem::size_of::<DisplayAccumulator>() as u64))
            .and_then(|n| n.checked_add((policy.native_edge() * policy.native_edge() * 4) as u64))
            .ok_or_else(|| resource("Source7 frame fixed storage"))?;
        let evaluation = existing
            .checked_add(retained)
            .and_then(|n| n.checked_add(frame.peak))
            .and_then(|n| n.checked_add(fixed))
            .ok_or_else(|| resource("Source7 evaluation peak"))?;
        let peak = evaluation.max(
            preparation
                .checked_add(existing)
                .ok_or_else(|| resource("Source7 preparation peak"))?,
        );
        if peak > job.workspace().bytes_limit() {
            return Err(resource("Source7 workspace peak"));
        }
        let work = frame.work.checked_add(WorkComponents {
            base: compile,
            noise: 0,
        })?;
        job.check_available(work)?;
        check()?;
        Ok(PreparedQuote {
            frame,
            work,
            peak,
            fixed,
        })
    }
    pub fn prepare(
        doc: &Document,
        request: DisplayRequest,
        settings: (&OpenJob, SlicePolicy),
        output_scope: &BudgetScope,
        binding: Value,
        q: PreparedQuote,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let (job, policy) = settings;
        job.check_available(q.work)?;
        if q.peak > job.workspace().bytes_limit() {
            return Err(resource("Source7 inherited byte limit"));
        }
        let job = job.prepare_document(doc, check)?;
        job.spend_control(q.frame.other)?;
        let mut fixed = job.workspace().scope().account();
        fixed.resize(q.fixed)?;
        let mut sums = Vec::new();
        sums.try_reserve_exact((PATCH * PATCH) as usize)
            .map_err(|_| resource("Source7 accumulator allocation"))?;
        let actual_fixed = (sums.capacity() as u64)
            .checked_mul(std::mem::size_of::<DisplayAccumulator>() as u64)
            .and_then(|n| n.checked_add((policy.native_edge() * policy.native_edge() * 4) as u64))
            .ok_or_else(|| resource("Source7 accumulator capacity"))?;
        let actual_peak = job
            .workspace()
            .scope()
            .footprint()
            .live
            .checked_sub(q.fixed)
            .and_then(|n| n.checked_add(q.frame.peak))
            .and_then(|n| n.checked_add(actual_fixed))
            .ok_or_else(|| resource("Source7 actual frame peak"))?;
        if actual_peak > job.workspace().bytes_limit() {
            return Err(resource("Source7 actual workspace capacity"));
        }
        fixed.resize(actual_fixed)?;
        sums.resize((PATCH * PATCH) as usize, DisplayAccumulator::default());
        let [w, h] = request.size();
        let bytes = u64::from(w)
            .checked_mul(u64::from(h))
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| resource("Source7 output extent"))?;
        let mut output = output_scope.account();
        output.resize(bytes)?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(bytes as usize)
            .map_err(|_| resource("Source7 output allocation"))?;
        if pixels.capacity() as u64 > OUTPUT_PIXELS_MAX * 4 {
            return Err(resource("Source7 output capacity"));
        }
        output.resize(pixels.capacity() as u64)?;
        pixels.resize(bytes as usize, 0);
        check()?;
        Ok(Self {
            pixels,
            sums,
            binding,
            request,
            job,
            native: None,
            column: 0,
            row: 0,
            policy,
            other_used: 0,
            other_limit: q.frame.other,
            _scratch: fixed,
            output,
            delivered: false,
            remaining: WorkComponents {
                base: q
                    .frame
                    .work
                    .base
                    .checked_sub(q.frame.other)
                    .ok_or_else(|| resource("Source7 deferred work"))?,
                noise: q.frame.work.noise,
            },
            total_work: q.work,
            mask_actual: q.frame.mask_actual,
            mask_upper: q.frame.mask_upper,
            copy: None,
            encoded_account: None,
        })
    }
    pub fn complete(&self) -> bool {
        self.row >= self.request.size()[1]
    }
    pub fn step(&mut self, doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<bool> {
        check()?;
        if self.complete() {
            return Ok(true);
        }
        let area = quote::patch(self.request, self.column, self.row);
        let (x, y) = self.native.unwrap_or_else(|| {
            self.sums.fill(DisplayAccumulator::default());
            (area.x0, area.y0)
        });
        let tile = Rect {
            x0: x,
            y0: y,
            x1: (x + self.policy.native_edge()).min(area.x1),
            y1: (y + self.policy.native_edge()).min(area.y1),
        };
        let q = quote::tile_quote(
            doc,
            tile,
            quote::resolve(self.request, area, tile),
            self.policy,
            check,
        )?;
        self.other_used = self
            .other_used
            .checked_add(q.other)
            .filter(|n| *n <= self.other_limit)
            .ok_or_else(|| resource("Source7 composition preflight changed"))?;
        let pixels = self
            .job
            .run(|| paintcraft_render::render_region_cancellable(doc, tile, check))?;
        if pixels.capacity() > (self.policy.native_edge() * self.policy.native_edge() * 4) as usize
        {
            return Err(resource("Source7 tile allocation exceeds slot"));
        }
        let step = i64::from(self.request.step());
        for (row, values) in pixels.chunks_exact(tile.width() as usize * 4).enumerate() {
            check()?;
            let sy = (tile.y0 + row as i64 - area.y0) / step;
            for (column, pixel) in values.as_chunks::<4>().0.iter().enumerate() {
                if column % 64 == 0 {
                    check()?;
                }
                let sx = (tile.x0 + column as i64 - area.x0) / step;
                self.sums[sy as usize * PATCH as usize + sx as usize]
                    .add(paintcraft_core::types::Rgba(*pixel))?;
            }
        }
        let (nx, ny) = if tile.x1 >= area.x1 {
            (area.x0, tile.y1)
        } else {
            (tile.x1, y)
        };
        if ny < area.y1 {
            self.native = Some((nx, ny));
            check()?;
            return Ok(false);
        }
        let [width, _] = self.request.size();
        let w = ((area.width() + step - 1) / step) as usize;
        let h = ((area.height() + step - 1) / step) as usize;
        for y in 0..h {
            check()?;
            for x in 0..w {
                let sum = &self.sums[y * PATCH as usize + x];
                let expected = (step.min(area.width() - x as i64 * step)
                    * step.min(area.height() - y as i64 * step))
                    as u64;
                if sum.samples() != expected {
                    return Err(bad("Source7 native samples changed"));
                }
                let dst = ((self.row as usize + y) * width as usize + self.column as usize + x) * 4;
                self.pixels[dst..dst + 4].copy_from_slice(&sum.resolve()?.0);
            }
        }
        self.native = None;
        self.column += PATCH;
        if self.column >= width {
            self.column = 0;
            self.row += PATCH;
        }
        if self.complete() && self.other_used != self.other_limit {
            return Err(resource("Source7 final composition count changed"));
        }
        check()?;
        Ok(self.complete())
    }
}
