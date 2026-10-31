//! Admitted source-local layer appearance regions. Source pixels stay immutable.
//! No process-default or per-tile budget is created here. Hosts supply the same
//! job scope to all concurrent partitions and charge final output separately.
use paintcraft_core::types::{Error, ErrorCode, Rect, Result, Rgba, resources::BudgetScope};
use paintcraft_document::Layer;

mod context;
mod plan;
mod tiles;
mod workspace;
pub use context::{capture_workspace_or_current, with_workspace, with_workspace_or_current};
pub(crate) use context::{captured, current};
pub use plan::{LayerAppearancePlan, WorkspaceQuote};
pub use tiles::{TiledQuote, quote_layer};
pub(crate) use tiles::{preflight, visit};

// Attachment tagging is excluded Rust factory bookkeeping, not payload.
#[derive(Clone, Debug)]
enum ProgramBinding {
    Legacy(std::sync::Arc<crate::prepared::PreparedPointJob>),
    Curves(std::sync::Arc<crate::prepared::PreparedCurvesJob>),
}
/// Shared intermediate-storage admission, independent of final output/cache
/// admission. A reserved job scope can back this factory without double-charging.
#[derive(Clone, Debug)]
pub struct WorkspaceAdmission {
    scope: BudgetScope,
    bytes_limit: u64,
    work_limit: u64,
    work_used: std::sync::Arc<std::sync::Mutex<u64>>,
    levels: Option<std::sync::Arc<crate::prepared::PreparedLevelsJob>>,
    programs: Option<ProgramBinding>,
}
impl WorkspaceAdmission {
    pub fn new(scope: BudgetScope, bytes_limit: u64, work_limit: u64) -> Self {
        Self {
            scope,
            bytes_limit,
            work_limit,
            work_used: std::sync::Arc::new(std::sync::Mutex::new(0)),
            levels: None,
            programs: None,
        }
    }
    /// Immutable programs captured for this frozen rendering invocation.
    pub fn prepared_levels(&self) -> Option<&crate::prepared::PreparedLevelsJob> {
        match &self.programs {
            Some(ProgramBinding::Legacy(job)) => job.levels(),
            Some(ProgramBinding::Curves(job)) => job.legacy().and_then(|old| old.levels()),
            None => self.levels.as_deref(),
        }
    }
    pub fn prepared_programs(&self) -> Option<&crate::prepared::PreparedPointJob> {
        match &self.programs {
            Some(ProgramBinding::Legacy(job)) => Some(job),
            Some(ProgramBinding::Curves(job)) => job.legacy(),
            None => None,
        }
    }
    pub fn prepared_curves(&self) -> Option<&crate::prepared::PreparedCurvesJob> {
        match &self.programs {
            Some(ProgramBinding::Curves(job)) => Some(job),
            _ => None,
        }
    }
    /// Includes legacy Levels-only and Curves-only frozen complete owners.
    pub fn has_prepared_programs(&self) -> bool {
        self.programs.is_some() || self.levels.is_some()
    }
    /// Actual retained logical program payload, not allocator/Arc bookkeeping.
    pub fn prepared_retained_bytes(&self) -> u64 {
        if let Some(job) = self.prepared_curves() {
            return job.retained_bytes();
        }
        self.prepared_programs().map_or_else(
            || {
                self.prepared_levels()
                    .map_or(0, crate::prepared::PreparedLevelsJob::retained_bytes)
            },
            crate::prepared::PreparedPointJob::retained_bytes,
        )
    }
    pub(crate) fn with_programs(
        mut self,
        job: std::sync::Arc<crate::prepared::PreparedPointJob>,
    ) -> Self {
        self.levels = None;
        self.programs = Some(ProgramBinding::Legacy(job));
        self
    }
    pub(crate) fn with_curves(
        mut self,
        job: std::sync::Arc<crate::prepared::PreparedCurvesJob>,
    ) -> Self {
        self.levels = None;
        self.programs = Some(ProgramBinding::Curves(job));
        self
    }
    pub(crate) fn with_levels(
        mut self,
        job: std::sync::Arc<crate::prepared::PreparedLevelsJob>,
    ) -> Self {
        self.levels = Some(job);
        self
    }
    pub(crate) fn with_byte_limit(mut self, bytes: u64) -> Self {
        self.bytes_limit = self.bytes_limit.min(bytes);
        self
    }
    pub fn scope(&self) -> &BudgetScope {
        &self.scope
    }
    pub fn bytes_limit(&self) -> u64 {
        self.bytes_limit
    }
    pub fn work_limit(&self) -> u64 {
        self.work_limit
    }
    /// Cumulative conservative work for this job, shared by cloned factories.
    /// A new job creates a new admission in the same shared byte-budget scope.
    pub fn work_used(&self) -> u64 {
        *self
            .work_used
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
    /// Spend modeled work on this same cumulative ledger. Clones share the
    /// counter; this method cannot reset it, refund work or enlarge its ceiling.
    pub fn reserve_work(&self, work: u64) -> Result<()> {
        let mut used = self
            .work_used
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let total = used
            .checked_add(work)
            .filter(|total| *total <= self.work_limit)
            .ok_or_else(|| limit("Spatial job aggregate work exceeds its limit"))?;
        *used = total;
        Ok(())
    }
}

pub fn active(layer: &Layer) -> bool {
    layer
        .filters
        .iter()
        .any(|filter| filter.enabled() && filter.descriptor().is_spatial())
}
pub(crate) fn relevant(doc: &paintcraft_document::Document, region: Rect) -> bool {
    doc.layers_in_order().any(|layer| {
        layer.visible
            && layer.opacity != 0
            && active(layer)
            && region.intersect(&layer.extent()).is_some()
    })
}

fn limit(message: &'static str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn invalid(message: &'static str) -> Error {
    Error::new(ErrorCode::InputSchema, message)
}
fn pixels(rect: Rect) -> Result<usize> {
    let width = rect
        .x1
        .checked_sub(rect.x0)
        .filter(|value| *value > 0)
        .ok_or_else(|| invalid("Empty or overflowing appearance rectangle"))?;
    let height = rect
        .y1
        .checked_sub(rect.y0)
        .filter(|value| *value > 0)
        .ok_or_else(|| invalid("Empty or overflowing appearance rectangle"))?;
    usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| limit("Appearance pixel count overflows"))
}

impl LayerAppearancePlan<'_> {
    /// Evaluate this frozen plan into caller-owned tightly packed output. The
    /// output may be partial on cancellation/error and must then be discarded.
    /// Every intermediate allocation is charged before source extraction.
    pub fn evaluate_into(
        &self,
        admission: &WorkspaceAdmission,
        output: &mut [Rgba],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if output.len() != pixels(self.output)? {
            return Err(invalid("Appearance output length differs from its region"));
        }
        if self.quote.bytes > admission.bytes_limit || self.quote.work > admission.work_limit {
            return Err(limit(
                "Spatial appearance exceeds its workspace or work limit",
            ));
        }
        self.with_pixels(admission, check, |pixels| {
            for (source, output) in pixels.chunks(64).zip(output.chunks_mut(64)) {
                check()?;
                output.copy_from_slice(source);
            }
            check()
        })
    }

    /// Borrow a completed appearance while its actual workspace charge remains
    /// live. The callback cannot retain the borrowed pixels beyond this scope.
    pub fn with_pixels<T>(
        &self,
        admission: &WorkspaceAdmission,
        check: &dyn Fn() -> Result<()>,
        consume: impl FnOnce(&[Rgba]) -> Result<T>,
    ) -> Result<T> {
        if self.quote.bytes > admission.bytes_limit || self.quote.work > admission.work_limit {
            return Err(limit(
                "Spatial appearance exceeds its workspace or work limit",
            ));
        }
        crate::prepared::validate_layer_programs(self.layer, admission, check)?;
        check()?;
        admission.reserve_work(self.quote.work)?;
        let mut workspace = workspace::Workspace::new(self, admission, check)?;
        workspace.extract(self, check)?;
        workspace.evaluate(self, admission, check)?;
        check()?;
        let result = consume(workspace.result(pixels(self.output)?))?;
        check()?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
