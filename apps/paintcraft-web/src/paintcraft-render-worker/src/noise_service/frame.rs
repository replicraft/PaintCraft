use super::*;
use paintcraft_core::types::Rect;
use paintcraft_render::{
    display::DisplayAccumulator,
    noise_work::{NoiseAdmission, NoiseJobPolicy, capture_noise_or_current, with_noise_workspace},
};
use quote::{FrameQuote, PATCH};
/// New type; old Frame/GenericFrame retain all original fields and byte layouts.
#[derive(Debug)]
pub(super) struct NoiseFrame {
    pub pixels: Vec<u8>,
    sums: Vec<DisplayAccumulator>,
    pub binding: Value,
    request: DisplayRequest,
    pub admission: NoiseAdmission,
    native: Option<(i64, i64)>,
    column: u32,
    row: u32,
    policy: SlicePolicy,
    other_used: u64,
    other_limit: u64,
    _scratch: BudgetAccount,
    pub output: BudgetAccount,
    pub delivered: bool,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct PreparedQuote {
    pub frame: FrameQuote,
    pub work: WorkComponents,
    pub peak: u64,
    pub fixed: u64,
}
impl NoiseFrame {
    /// Metadata only: no sidecar, compilation or pixels. An already installed
    /// matching pair must also have room and the frozen complete prepared keys.
    pub fn quote(
        doc: &Document,
        request: DisplayRequest,
        profile: wire::Profile,
        policy: SlicePolicy,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<PreparedQuote> {
        let frame = quote::frame_quote(doc, request, profile, policy, check)?;
        let programs = paintcraft_render::prepared::quote_document_programs(doc, check)?;
        let controls = NoiseAdmission::control_quote()?;
        let fixed = (PATCH as u64)
            .checked_mul(PATCH as u64)
            .and_then(|n| n.checked_mul(std::mem::size_of::<DisplayAccumulator>() as u64))
            .and_then(|n| n.checked_add((policy.native_edge() * policy.native_edge() * 4) as u64))
            .ok_or_else(|| resource("Source6 frame storage"))?;
        let mut compile = programs.compile_work;
        let mut retained = programs.retained_bytes;
        let mut preparation = programs.preparation_peak_bytes;
        let absent = std::cell::Cell::new(false);
        // Capture refuses a legacy-only/foreign caller; only actual absence is
        // allowed to proceed quote-only without constructing a new job.
        let installed = paintcraft_render::noise_work::capture_noise_or_current(
            || {
                absent.set(true);
                Err(no_caller())
            },
            profile.workload.limits(),
        );
        let inherited = match installed {
            Ok(parent) => {
                parent.validate_document(doc, check)?;
                if parent.workspace().has_prepared_programs() {
                    compile = 0;
                    preparation = 0;
                    retained = parent.workspace().prepared_retained_bytes();
                }
                Some(parent)
            }
            Err(_) if absent.get() => None,
            Err(error) => return Err(error),
        };
        let work = frame.work.checked_add(WorkComponents {
            base: compile,
            noise: 0,
        })?;
        profile.workload.check(work)?;
        let effective_bytes = if let Some(parent) = inherited {
            parent.check_available(work)?;
            parent.limits().bytes
        } else {
            profile.workload.limits().bytes
        };
        let evaluation = retained
            .checked_add(frame.peak)
            .and_then(|n| n.checked_add(fixed))
            .and_then(|n| n.checked_add(controls.counter_bytes))
            .ok_or_else(|| resource("Source6 evaluation peak"))?;
        let peak = evaluation.max(
            preparation
                .checked_add(controls.counter_bytes)
                .ok_or_else(|| resource("Source6 preparation peak"))?,
        );
        if peak > effective_bytes {
            return Err(resource("Source6 frame exceeds workspace"));
        }
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
        settings: (wire::Profile, SlicePolicy),
        scopes: (BudgetScope, &BudgetScope),
        binding: Value,
        q: PreparedQuote,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let (profile, policy) = settings;
        let (scope, output_scope) = scopes;
        check()?;
        let admission = capture_noise_or_current(
            || {
                NoiseAdmission::new_root(
                    scope,
                    NoiseJobPolicy::BrowserV1,
                    profile.workload.limits(),
                )
            },
            profile.workload.limits(),
        )?;
        admission.check_available(q.work)?;
        if q.peak > admission.limits().bytes {
            return Err(resource("Source6 inherited byte ceiling"));
        }
        let admission = admission.prepare_document(doc, check)?;
        admission.workspace().reserve_work(q.frame.other)?;
        let mut fixed = admission.workspace().scope().account();
        fixed.resize(q.fixed)?;
        let mut sums = Vec::new();
        sums.try_reserve_exact((PATCH * PATCH) as usize)
            .map_err(|_| resource("Source6 accumulators"))?;
        let actual_fixed = (sums.capacity() as u64)
            .checked_mul(std::mem::size_of::<DisplayAccumulator>() as u64)
            .and_then(|n| n.checked_add((policy.native_edge() * policy.native_edge() * 4) as u64))
            .ok_or_else(|| resource("Source6 accumulator capacity"))?;
        let actual_peak = admission
            .workspace()
            .prepared_retained_bytes()
            .checked_add(q.frame.peak)
            .and_then(|n| n.checked_add(actual_fixed))
            .and_then(|n| n.checked_add(NoiseAdmission::control_quote().ok()?.counter_bytes))
            .ok_or_else(|| resource("Source6 actual frame capacity"))?;
        if actual_peak > admission.limits().bytes {
            return Err(resource("Source6 actual inherited byte ceiling"));
        }
        fixed.resize(actual_fixed)?;
        sums.resize((PATCH * PATCH) as usize, DisplayAccumulator::default());
        let [w, h] = request.size();
        let bytes = u64::from(w)
            .checked_mul(u64::from(h))
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| resource("Source6 output size"))?;
        let mut output = output_scope.account();
        output.resize(bytes)?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(bytes as usize)
            .map_err(|_| resource("Source6 output allocation"))?;
        if pixels.capacity() as u64 > OUTPUT_PIXELS_MAX * 4 {
            return Err(resource("Source6 unexpected output capacity"));
        }
        output.resize(pixels.capacity() as u64)?;
        pixels.resize(bytes as usize, 0);
        check()?;
        Ok(Self {
            pixels,
            sums,
            binding,
            request,
            admission,
            native: None,
            column: 0,
            row: 0,
            policy,
            other_used: 0,
            other_limit: q.frame.other,
            _scratch: fixed,
            output,
            delivered: false,
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
            .ok_or_else(|| resource("Source6 composition preflight changed"))?;
        let pixels = with_noise_workspace(self.admission.clone(), || {
            paintcraft_render::render_region_cancellable(doc, tile, check)
        })?;
        if pixels.capacity() > (self.policy.native_edge() * self.policy.native_edge() * 4) as usize
        {
            return Err(resource("Source6 tile allocation exceeds slot"));
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
                    return Err(bad("Source6 native samples changed"));
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
            return Err(resource("Source6 final composition count changed"));
        }
        check()?;
        Ok(self.complete())
    }
}
fn no_caller() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Source6 quote has no installed caller",
    )
}
