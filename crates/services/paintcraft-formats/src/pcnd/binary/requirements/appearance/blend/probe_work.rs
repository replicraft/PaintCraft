//! One defining supplemental policy for the unchanged Source8 borrowed probe.
use super::CONTROL_BYTES;
use paintcraft_core::types::Result;
use std::mem::size_of;
/// Scalar probe workload only, not validation, budget, source or adoption authority.
/// Metadata means encoded metadata bytes EXCLUDING only bulk ICC profile bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source8ProbeWorkFacts {
    /// All stored layer roots, including hidden layers.
    pub layers: u64,
    /// All stored filters, including disabled filters.
    pub filters: u64,
    /// Present masks, including disabled and empty masks.
    pub masks: u64,
    /// Aggregate encoded coverage bands.
    pub bands: u64,
    /// Aggregate encoded coverage runs.
    pub runs: u64,
    /// Encoded metadata excluding bulk ICC profile bytes.
    pub metadata: u64,
}
mod work {
    use paintcraft_core::types::{Error, ErrorCode, Result};
    fn limit() -> Error {
        Error::new(ErrorCode::ResourceLimit, "Source8 assembly bound")
    }
    pub(super) fn add(a: u64, b: u64) -> Result<u64> {
        a.checked_add(b).ok_or_else(limit)
    }
    pub(super) fn mul(a: u64, b: u64) -> Result<u64> {
        a.checked_mul(b).ok_or_else(limit)
    }
    pub(super) fn sum(v: &[u64]) -> Result<u64> {
        v.iter().try_fold(0, |a, &b| add(a, b))
    }
    pub(super) fn bare(n: u64) -> Result<u64> {
        if n < 2 {
            return Ok(0);
        }
        add(
            mul(add(n / 2, n - 1)?, u64::from(64 - n.leading_zeros()) + 1)?,
            n - 1,
        )
    }
    pub(super) fn limit_public() -> Error {
        limit()
    }
}
/// Pure logical units around the existing probe's raw sorts/descriptor/hash work.
/// Caller MUST prepay its quotation formation and controls BEFORE calling this,
/// then debit this result BEFORE the actual probe. Existing helper work/checks
/// remain separate; pass its raw check, without adding512 twice. The successful
/// supported-target path performs no allocation, account, lock, source traversal
/// or callback. Refusal constructs the existing owned Error diagnostic.
///
/// # Errors
/// Returns ResourceLimit for an unsupported pointer width or arithmetic overflow.
pub fn source8_probe_supplemental_work_quote(f: Source8ProbeWorkFacts) -> Result<u64> {
    let w = size_of::<usize>();
    let native = w == 8;
    if !matches!(w, 4 | 8) {
        return Err(work::limit_public());
    }
    let registry_sort = if native { 330880 } else { 255680 };
    let index = if native { 24 } else { 20 };
    let headers = (b"paintcraft.worker-coverage-bounds/v1\0".len()
        + 2
        + if native { 11 } else { 9 }
        + 2
        + 4
        + b"paintcraft.worker-mask-inventory/v1\0".len()
        + 4
        + b"paintcraft.worker-layer-masks/v1\0".len()
        + 4) as u64;
    let canonical = work::sum(&[
        work::mul(62, f.masks)?,
        work::mul(24, f.bands)?,
        work::mul(17, f.runs)?,
    ])?;
    work::sum(&[
        work::mul(16, CONTROL_BYTES)?,
        work::mul(208, f.metadata)?,
        work::mul(512, work::add(f.layers, 1)?)?,
        work::mul(
            128,
            work::sum(&[2, f.layers, f.filters, work::mul(6, f.masks)?])?,
        )?,
        work::mul(
            512,
            work::add(
                work::bare(f.layers)?,
                work::bare(f.filters)?.min(work::mul(16, f.filters)?),
            )?,
        )?,
        work::mul(3 * 1706, f.filters)?,
        1706 * 13 + registry_sort + 1280 + 64 * 13,
        work::mul(4 * index, work::add(f.layers, f.filters)?)?,
        work::mul(3856, f.masks)?,
        work::add(
            work::mul(
                16,
                work::add(work::mul(3, canonical)?, work::mul(384, f.masks)?)?,
            )?,
            work::mul(1440, f.masks)?,
        )?,
        work::add(
            work::mul(16, work::sum(&[headers, work::mul(327, f.masks)?, 384])?)?,
            1440,
        )?,
        2528,
    ])
}
