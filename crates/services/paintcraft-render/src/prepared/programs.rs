//! Mixed-family capture. The no-active-Exposure path delegates legacy Levels.
use super::{PreparedPointJob, WorkspaceAdmission, limit};
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    filters::{Descriptor, ExposureSrgbV1, LevelsRgbV1},
};
use paintcraft_document::{Document, Layer};
use std::sync::Arc;
const CAPTURE_BYTES: u64 = (PreparedPointJob::MAX_PROGRAMS
    * (std::mem::size_of::<LevelsRgbV1>() + std::mem::size_of::<ExposureSrgbV1>()))
    as u64;
fn active_exposure(doc: &Document) -> bool {
    doc.layers.values().any(|l| {
        l.filters
            .iter()
            .any(|f| f.enabled() && matches!(f.descriptor(), Descriptor::ExposureSrgbV1(_)))
    })
}
// Forward guard only: registration alone never enables an unprepared family.
pub(super) fn unprepared_curves() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Curves appearance requires its frozen prepared policy",
    )
}
pub(super) fn refuse_layer_curves(layer: &Layer) -> Result<()> {
    if layer
        .filters
        .iter()
        .any(|f| f.enabled() && matches!(f.descriptor(), Descriptor::CurvesRgbV1(_)))
    {
        return Err(unprepared_curves());
    }
    Ok(())
}
pub(super) fn refuse_document_curves(doc: &Document) -> Result<()> {
    for layer in doc.layers.values() {
        refuse_layer_curves(layer)?;
    }
    Ok(())
}
fn missing() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Exposure appearance requires its frozen complete-key program job",
    )
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DocumentProgramsQuote {
    pub levels_programs: usize,
    pub exposure_programs: usize,
    pub unique_programs: usize,
    /// Conservative abstract preparation allowance. Raw family/word counters
    /// remain tagged in each owner; their costs are not claimed interchangeable.
    pub compile_work: u64,
    pub preparation_peak_bytes: u64,
    pub retained_bytes: u64,
}
impl DocumentProgramsQuote {
    /// Derived count; the old quote layout/fields and legacy values stay exact.
    pub fn curves_programs(self) -> usize {
        self.unique_programs
            .saturating_sub(self.levels_programs.saturating_add(self.exposure_programs))
    }
}
fn capture(
    doc: &Document,
    levels: &mut [LevelsRgbV1; PreparedPointJob::MAX_PROGRAMS],
    exposures: &mut [ExposureSrgbV1; PreparedPointJob::MAX_PROGRAMS],
    check: &dyn Fn() -> Result<()>,
) -> Result<(usize, usize)> {
    let (mut l, mut e) = (0, 0);
    for layer in doc.layers.values() {
        check()?;
        for f in layer.filters.iter().filter(|f| f.enabled()) {
            check()?;
            match f.descriptor() {
                Descriptor::LevelsRgbV1(p) => {
                    if levels[..l].contains(&p) {
                        continue;
                    }
                    if l + e == PreparedPointJob::MAX_PROGRAMS {
                        return Err(limit(
                            "Prepared complete document key count exceeds host bound",
                        ));
                    }
                    levels[l] = p;
                    l += 1;
                }
                Descriptor::ExposureSrgbV1(p) => {
                    if exposures[..e].contains(&p) {
                        continue;
                    }
                    if l + e == PreparedPointJob::MAX_PROGRAMS {
                        return Err(limit(
                            "Prepared complete document key count exceeds host bound",
                        ));
                    }
                    exposures[e] = p;
                    e += 1;
                }
                _ => {}
            }
        }
    }
    // Numerical stack order is untouched. Only eager compilation order is fixed:
    // existing canonical Levels first, then Exposure's complete two-byte key.
    levels[..l].sort_unstable_by_key(|p| p.semantic_bytes());
    exposures[..e].sort_unstable_by_key(|p| p.semantic_bytes());
    Ok((l, e))
}
fn document_quote(l: &[LevelsRgbV1], e: &[ExposureSrgbV1]) -> Result<DocumentProgramsQuote> {
    let q = super::point_job::quote(l, e)?;
    Ok(DocumentProgramsQuote {
        levels_programs: q.levels,
        exposure_programs: q.exposures,
        unique_programs: q.levels + q.exposures,
        compile_work: q.compile_work,
        preparation_peak_bytes: CAPTURE_BYTES
            .checked_add(q.peak_bytes)
            .ok_or_else(|| limit("Prepared document capture/peak sum overflows"))?,
        retained_bytes: q.retained_bytes,
    })
}
/// Fresh-inventory metadata quote only, never compilation/admission. Its fixed
/// two<=256-key stack arrays are excluded for this query; actual preparation
/// charges and retains both throughout all compilation. Bound-job retention is
/// available separately through WorkspaceAdmission::prepared_retained_bytes.
/// With no active Exposure, values/callbacks are exactly the old Levels quote.
pub(super) fn quote_legacy_programs(
    doc: &Document,
    check: &dyn Fn() -> Result<()>,
) -> Result<DocumentProgramsQuote> {
    refuse_document_curves(doc)?;
    if !active_exposure(doc) {
        let q = super::quote_document_levels(doc, check)?;
        return Ok(DocumentProgramsQuote {
            levels_programs: q.unique_programs,
            exposure_programs: 0,
            unique_programs: q.unique_programs,
            compile_work: q.compile_work,
            preparation_peak_bytes: q.preparation_peak_bytes,
            retained_bytes: q.retained_bytes,
        });
    }
    let mut levels = [LevelsRgbV1::default(); PreparedPointJob::MAX_PROGRAMS];
    let mut exposures = [ExposureSrgbV1::default(); PreparedPointJob::MAX_PROGRAMS];
    let (l, e) = capture(doc, &mut levels, &mut exposures, check)?;
    document_quote(&levels[..l], &exposures[..e])
}
fn validate_doc(
    doc: &Document,
    admission: &WorkspaceAdmission,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    for layer in doc.layers.values() {
        check()?;
        validate_layer_programs(layer, admission, check)?;
    }
    Ok(())
}
/// Freeze all active unique complete keys in one caller-owned scope/counter.
/// Existing frozen jobs never extend. Missing Exposure under a legacy-only
/// Levels owner refuses; callers must begin a fresh final-document invocation.
pub(super) fn prepare_legacy_programs(
    doc: &Document,
    admission: WorkspaceAdmission,
    check: &dyn Fn() -> Result<()>,
) -> Result<WorkspaceAdmission> {
    refuse_document_curves(doc)?;
    if admission.prepared_programs().is_some() {
        validate_doc(doc, &admission, check)?;
        return Ok(admission);
    }
    if !active_exposure(doc) {
        return super::prepare_document_levels(doc, admission, check);
    }
    if admission.prepared_levels().is_some() {
        return Err(missing());
    }
    let bytes = admission
        .bytes_limit()
        .checked_sub(CAPTURE_BYTES)
        .ok_or_else(|| limit("Prepared document inventory scratch exceeds allowance"))?;
    check()?;
    let mut charge = admission.scope().account();
    charge.resize(CAPTURE_BYTES)?;
    let mut levels = [LevelsRgbV1::default(); PreparedPointJob::MAX_PROGRAMS];
    let mut exposures = [ExposureSrgbV1::default(); PreparedPointJob::MAX_PROGRAMS];
    let (l, e) = capture(doc, &mut levels, &mut exposures, check)?;
    let job = PreparedPointJob::new(
        &levels[..l],
        &exposures[..e],
        admission.clone().with_byte_limit(bytes),
        check,
    )?;
    check()?;
    Ok(admission.with_programs(Arc::new(job)))
}
pub(crate) fn validate_layer_programs(
    layer: &Layer,
    admission: &WorkspaceAdmission,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    // Keeps the existing Levels-only check order and callback behavior exact.
    super::validate_layer_levels(layer, admission, check)?;
    for f in layer.filters.iter().filter(|f| f.enabled()) {
        if let Descriptor::ExposureSrgbV1(p) = f.descriptor() {
            check()?;
            if admission
                .prepared_programs()
                .and_then(|job| job.exposure(p))
                .is_none()
            {
                return Err(missing());
            }
        }
    }
    super::curves_document::validate_layer_curves(layer, admission, check)?;
    Ok(())
}
/// Bind before all cache hits, parallel capture and GPU appearance preparation.
/// No TLS fallback realm exists. No active Exposure delegates the old path.
pub(super) fn with_legacy_programs<T>(
    doc: &Document,
    check: &dyn Fn() -> Result<()>,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    refuse_document_curves(doc)?;
    if !active_exposure(doc) {
        if let Some(admission) = crate::spatial::current()
            && admission.prepared_programs().is_some()
        {
            validate_doc(doc, &admission, check)?;
        }
        return super::with_document_levels(doc, check, work);
    }
    let admission = crate::spatial::current().ok_or_else(missing)?;
    let admission = prepare_legacy_programs(doc, admission, check)?;
    crate::spatial::with_workspace(admission, work)
}
#[cfg(test)]
use super::{prepare_document_programs, quote_document_programs, with_document_programs};
#[cfg(test)]
#[path = "program_tests.rs"]
mod tests;
