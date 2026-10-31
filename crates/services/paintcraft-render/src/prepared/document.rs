//! Freeze one eager complete-policy inventory inside an existing caller realm.
use super::{PreparedLevelsJob, WorkspaceAdmission, limit, quote_levels_job};
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    filters::{Descriptor, LevelsRgbV1},
};
use paintcraft_document::Document;
use std::sync::Arc;
const CAPTURE_BYTES: u64 =
    (PreparedLevelsJob::MAX_PROGRAMS * std::mem::size_of::<LevelsRgbV1>()) as u64;
fn active(doc: &Document) -> bool {
    doc.layers.values().any(|layer| {
        layer
            .filters
            .iter()
            .any(|f| f.enabled() && matches!(f.descriptor(), Descriptor::LevelsRgbV1(_)))
    })
}
fn missing() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Levels appearance requires its frozen prepared policy",
    )
}
/// Requested logical job payload for a frozen document. Actual pointer capacity
/// is reconciled by preparation; inspect the prepared job for actual retention.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DocumentLevelsQuote {
    pub unique_programs: usize,
    pub compile_work: u64,
    pub preparation_peak_bytes: u64,
    pub retained_bytes: u64,
}
fn capture(
    doc: &Document,
    policies: &mut [LevelsRgbV1; PreparedLevelsJob::MAX_PROGRAMS],
    check: &dyn Fn() -> Result<()>,
) -> Result<usize> {
    let mut len = 0;
    for layer in doc.layers.values() {
        check()?;
        for filter in layer.filters.iter().filter(|f| f.enabled()) {
            check()?;
            if let Descriptor::LevelsRgbV1(policy) = filter.descriptor() {
                if policies[..len].contains(&policy) {
                    continue;
                }
                if len == policies.len() {
                    return Err(limit(
                        "Levels unique document policy count exceeds host bound",
                    ));
                }
                policies[len] = policy;
                len += 1;
            }
        }
    }
    // HashMap layer iteration is not a wire/resource identity. Both quote and
    // preparation consume this same canonical complete six-byte policy order.
    policies[..len].sort_unstable_by_key(|policy| policy.semantic_bytes());
    Ok(len)
}
/// Complete policies use canonical semantic-byte order, independent of layer maps.
/// Metadata-only, nonallocating quote. Its fixed <=256-policy stack buffer is
/// explicitly excluded from execution admission for this query; preparation
/// includes and charges that SAME inventory storage in preparation_peak_bytes.
/// No table is built and no work counter is created or spent by this quote.
pub fn quote_document_levels(
    doc: &Document,
    check: &dyn Fn() -> Result<()>,
) -> Result<DocumentLevelsQuote> {
    let mut policies = [LevelsRgbV1::default(); PreparedLevelsJob::MAX_PROGRAMS];
    let len = capture(doc, &mut policies, check)?;
    if len == 0 {
        return Ok(DocumentLevelsQuote::default());
    }
    let quote = quote_levels_job(&policies[..len])?;
    Ok(DocumentLevelsQuote {
        unique_programs: len,
        compile_work: quote.compile_work,
        preparation_peak_bytes: CAPTURE_BYTES
            .checked_add(quote.peak_bytes)
            .ok_or_else(|| limit("Levels document preparation peak overflows"))?,
        retained_bytes: quote
            .index_bytes
            .checked_add((len as u64) * super::LEVELS_TABLE_BYTES)
            .ok_or_else(|| limit("Levels document retained quote overflows"))?,
    })
}

/// Prepare active Levels across all layers, deduplicating before the unique-key
/// host bound. Reuse a bound job only if every required complete key is present;
/// a missing key never triggers implicit extension or compilation in tile paths.
/// No new byte realm or work counter is created. Inventory scratch is charged
/// while it coexists with eager compilation, then released before rendering.
pub fn prepare_document_levels(
    doc: &Document,
    admission: WorkspaceAdmission,
    check: &dyn Fn() -> Result<()>,
) -> Result<WorkspaceAdmission> {
    if !active(doc) {
        return Ok(admission);
    }
    if admission.prepared_programs().is_some() {
        // An Exposure-only frozen job must not acquire an ignored legacy
        // Levels attachment, even when a caller enters through this old API.
        for layer in doc.layers.values() {
            check()?;
            validate_layer_levels(layer, &admission, check)?;
        }
        return Ok(admission);
    }
    if let Some(job) = admission.prepared_levels() {
        for layer in doc.layers.values() {
            check()?;
            for filter in layer.filters.iter().filter(|f| f.enabled()) {
                check()?;
                if let Descriptor::LevelsRgbV1(policy) = filter.descriptor()
                    && job.get(policy).is_none()
                {
                    return Err(missing());
                }
            }
        }
        return Ok(admission);
    }
    let bytes = admission
        .bytes_limit()
        .checked_sub(CAPTURE_BYTES)
        .ok_or_else(|| limit("Levels inventory scratch exceeds caller allowance"))?;
    check()?;
    let mut charge = admission.scope().account();
    charge.resize(CAPTURE_BYTES)?;
    let mut policies = [LevelsRgbV1::default(); PreparedLevelsJob::MAX_PROGRAMS];
    let len = capture(doc, &mut policies, check)?;
    let job = PreparedLevelsJob::new(
        &policies[..len],
        admission.clone().with_byte_limit(bytes),
        check,
    )?;
    check()?;
    Ok(admission.with_levels(Arc::new(job)))
}
pub(crate) fn validate_layer_levels(
    layer: &paintcraft_document::Layer,
    admission: &WorkspaceAdmission,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    for filter in layer.filters.iter().filter(|filter| filter.enabled()) {
        if let Descriptor::LevelsRgbV1(policy) = filter.descriptor() {
            check()?;
            if admission
                .prepared_levels()
                .and_then(|job| job.get(policy))
                .is_none()
            {
                return Err(missing());
            }
        }
    }
    Ok(())
}
/// Bind at an OUTER immutable document entry before cache hits/output/tiles.
/// Nested entries validate and reuse the same job. Standalone explicit region
/// plans instead consume the attached job on their supplied admission directly.
/// With no active Levels this preserves the prior callback/budget path exactly.
pub fn with_document_levels<T>(
    doc: &Document,
    check: &dyn Fn() -> Result<()>,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    if !active(doc) {
        return work();
    }
    let admission = crate::spatial::current().ok_or_else(missing)?;
    let admission = prepare_document_levels(doc, admission, check)?;
    crate::spatial::with_workspace(admission, work)
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
