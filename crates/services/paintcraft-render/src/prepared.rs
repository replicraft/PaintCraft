//! Admitted derived point programs. This adds no document/filter capability.
use crate::spatial::WorkspaceAdmission;
use paintcraft_core::types::{
    Error, ErrorCode, Result, Rgba, filters::LevelsRgbV1, resources::BudgetAccount,
};
use paintcraft_effects::{levels, mix_point_coverage};
mod compiler;
pub use compiler::{CompileWork, LevelsBackend};

pub const LEVELS_TABLE_BYTES: u64 = 256;

/// Logical payload storage, excluding caller input/output and Rust bookkeeping.
/// Scratch is released after preparation; the table charge follows its owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelsQuote {
    pub backend: LevelsBackend,
    pub peak_bytes: u64,
    pub retained_bytes: u64,
    pub scratch_bytes: u64,
    pub compile_work: u64,
}

pub fn quote_levels(policy: LevelsRgbV1) -> LevelsQuote {
    compiler::quote(policy)
}

/// One immutable compiled table with its retained payload account. It is movable
/// and shareable through a caller-owned Arc, but copying it requires new admission.
/// No implicit process realm, cache, eviction policy or per-row compilation exists.
#[derive(Debug)]
pub struct PreparedLevels {
    policy: LevelsRgbV1,
    table: [u8; 256],
    actual_compile_work: CompileWork,
    // Storage fields precede the account, so their logical lifetime ends first.
    _charge: BudgetAccount,
}

fn limit(message: &'static str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}

impl PreparedLevels {
    /// Prepare once in the caller's existing shared render-job admission.
    /// Compilation work uses the same cumulative counter as spatial regions.
    /// The byte ceiling applies to this preparation; aggregate live payloads
    /// remain constrained by the caller's shared realm/reserved job scope.
    /// Failed/cancelled preparation returns no owner and releases both accounts;
    /// conservatively reserved work remains spent in that job, as for regions.
    pub fn new(
        policy: LevelsRgbV1,
        admission: &WorkspaceAdmission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<Self> {
        let quote = quote_levels(policy);
        if quote.peak_bytes > admission.bytes_limit() || quote.compile_work > admission.work_limit()
        {
            return Err(limit("Prepared Levels exceeds its byte/work allowance"));
        }
        check()?;
        admission.reserve_work(quote.compile_work)?;
        let mut scratch_charge = admission.scope().account();
        scratch_charge.resize(quote.scratch_bytes)?;
        let mut table_charge = admission.scope().account();
        table_charge.resize(quote.retained_bytes)?;
        check()?;
        let mut table = [0; 256];
        let used = compiler::build(quote.backend, policy, &mut table, check)?;
        drop(scratch_charge);
        check()?;
        Ok(Self {
            policy,
            table,
            actual_compile_work: used,
            _charge: table_charge,
        })
    }

    pub fn policy(&self) -> LevelsRgbV1 {
        self.policy
    }

    /// Tagged raw arithmetic units, not elapsed time or transferable quota.
    pub fn actual_compile_work(&self) -> CompileWork {
        self.actual_compile_work
    }

    pub fn backend(&self) -> LevelsBackend {
        match self.actual_compile_work {
            CompileWork::U32 { .. } => LevelsBackend::U32,
            CompileWork::U64 { .. } => LevelsBackend::U64,
        }
    }

    /// Apply the retained table with exactly one point-mask blend per pixel.
    /// Alpha and hidden RGB retain the encoded point-profile contract.
    /// Final output storage belongs to the caller and is charged separately.
    /// Geometry/admission failures before work leave output intact; cancellation
    /// during a row may leave private partial output, which must be discarded.
    pub fn evaluate_row(
        &self,
        admission: &WorkspaceAdmission,
        source: &[Rgba],
        coverage: Option<&[u8]>,
        output: &mut [Rgba],
        check: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if source.len() != output.len()
            || coverage.is_some_and(|coverage| coverage.len() != source.len())
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Prepared Levels row/coverage lengths differ",
            ));
        }
        let work = u64::try_from(source.len())
            .ok()
            .and_then(|count| count.checked_mul(2))
            .ok_or_else(|| limit("Prepared Levels row work overflows"))?;
        check()?;
        admission.reserve_work(work)?;
        for (chunk, (source, output)) in source.chunks(64).zip(output.chunks_mut(64)).enumerate() {
            check()?;
            for (offset, (incoming, output)) in source.iter().zip(output).enumerate() {
                let m = coverage.map_or(255, |coverage| coverage[chunk * 64 + offset]);
                *output = if m == 0 {
                    *incoming
                } else {
                    mix_point_coverage(*incoming, levels::evaluate_pixel(&self.table, *incoming), m)
                };
            }
        }
        check()
    }
}

#[cfg(test)]
mod tests;
