//! Curves-active gateway. No active Curves delegates the exact old9 path.
use super::{
    DocumentProgramsQuote, PreparedCurvesJob, PreparedPointJob, WorkspaceAdmission, limit,
};
use paintcraft_core::types::{
    Result,
    filters::{CurvesRgbV1, Descriptor, ExposureSrgbV1, LevelsRgbV1},
};
use paintcraft_document::{Document, Layer};
use std::sync::Arc;
const MAX: usize = PreparedPointJob::MAX_PROGRAMS;
const CAPTURE: u64 = (MAX
    * (std::mem::size_of::<LevelsRgbV1>()
        + std::mem::size_of::<ExposureSrgbV1>()
        + std::mem::size_of::<CurvesRgbV1>())) as u64;
fn active(doc: &Document) -> bool {
    doc.layers.values().any(|l| {
        l.filters
            .iter()
            .any(|f| f.enabled() && matches!(f.descriptor(), Descriptor::CurvesRgbV1(_)))
    })
}
fn capture(
    doc: &Document,
    l: &mut [LevelsRgbV1; MAX],
    e: &mut [ExposureSrgbV1; MAX],
    c: &mut [CurvesRgbV1; MAX],
    check: &dyn Fn() -> Result<()>,
) -> Result<(usize, usize, usize)> {
    let (mut nl, mut ne, mut nc) = (0, 0, 0);
    for layer in doc.layers.values() {
        check()?;
        for f in layer.filters.iter().filter(|f| f.enabled()) {
            check()?;
            match f.descriptor() {
                Descriptor::LevelsRgbV1(p) => {
                    if l[..nl].contains(&p) {
                        continue;
                    }
                    if nl + ne + nc == MAX {
                        return Err(limit("Prepared combined document key bound"));
                    }
                    l[nl] = p;
                    nl += 1;
                }
                Descriptor::ExposureSrgbV1(p) => {
                    if e[..ne].contains(&p) {
                        continue;
                    }
                    if nl + ne + nc == MAX {
                        return Err(limit("Prepared combined document key bound"));
                    }
                    e[ne] = p;
                    ne += 1;
                }
                Descriptor::CurvesRgbV1(p) => {
                    if c[..nc].contains(&p) {
                        continue;
                    }
                    if nl + ne + nc == MAX {
                        return Err(limit("Prepared combined document key bound"));
                    }
                    c[nc] = p;
                    nc += 1;
                }
                _ => {}
            }
        }
    }
    l[..nl].sort_unstable_by_key(|p| p.semantic_bytes());
    e[..ne].sort_unstable_by_key(|p| p.semantic_bytes());
    c[..nc].sort_unstable_by_key(|p| p.semantic_bytes());
    Ok((nl, ne, nc))
}
/// Metadata-only quote; fixed bounded capture arrays are excluded for this
/// query and explicitly charged throughout actual preparation. No compile occurs.
pub fn quote_document_programs(
    doc: &Document,
    check: &dyn Fn() -> Result<()>,
) -> Result<DocumentProgramsQuote> {
    if !active(doc) {
        return super::programs::quote_legacy_programs(doc, check);
    }
    quote_active(doc, check)
}
#[inline(never)]
fn quote_active(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<DocumentProgramsQuote> {
    let mut l = [LevelsRgbV1::default(); MAX];
    let mut e = [ExposureSrgbV1::default(); MAX];
    let mut c = [CurvesRgbV1::default(); MAX];
    let (nl, ne, nc) = capture(doc, &mut l, &mut e, &mut c, check)?;
    let q = super::curves_job::quote(&l[..nl], &e[..ne], &c[..nc])?;
    Ok(DocumentProgramsQuote {
        levels_programs: q.levels,
        exposure_programs: q.exposures,
        unique_programs: q.levels + q.exposures + q.curves,
        compile_work: q.work,
        preparation_peak_bytes: CAPTURE
            .checked_add(q.peak)
            .ok_or_else(|| limit("Prepared Curves capture peak overflow"))?,
        retained_bytes: q.retained,
    })
}
pub(super) fn validate_doc(
    doc: &Document,
    a: &WorkspaceAdmission,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    for layer in doc.layers.values() {
        check()?;
        super::programs::validate_layer_programs(layer, a, check)?;
    }
    Ok(())
}
pub(super) fn validate_layer_curves(
    layer: &Layer,
    a: &WorkspaceAdmission,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    for f in layer.filters.iter().filter(|f| f.enabled()) {
        if let Descriptor::CurvesRgbV1(p) = f.descriptor() {
            check()?;
            if a.prepared_curves().and_then(|j| j.get(p)).is_none() {
                return Err(super::programs::unprepared_curves());
            }
        }
    }
    Ok(())
}
/// Any bound complete owner is immutable; never attach a missing family/key.
pub fn prepare_document_programs(
    doc: &Document,
    a: WorkspaceAdmission,
    check: &dyn Fn() -> Result<()>,
) -> Result<WorkspaceAdmission> {
    if a.prepared_curves().is_some() {
        validate_doc(doc, &a, check)?;
        return Ok(a);
    }
    if !active(doc) {
        return super::programs::prepare_legacy_programs(doc, a, check);
    }
    if a.has_prepared_programs() {
        validate_doc(doc, &a, check)?;
        return Ok(a);
    }
    prepare_active(doc, a, check)
}
#[inline(never)]
fn prepare_active(
    doc: &Document,
    a: WorkspaceAdmission,
    check: &dyn Fn() -> Result<()>,
) -> Result<WorkspaceAdmission> {
    let bytes = a
        .bytes_limit()
        .checked_sub(CAPTURE)
        .ok_or_else(|| limit("Prepared Curves capture allowance"))?;
    check()?;
    let mut capture_charge = a.scope().account();
    capture_charge.resize(CAPTURE)?;
    let mut l = [LevelsRgbV1::default(); MAX];
    let mut e = [ExposureSrgbV1::default(); MAX];
    let mut c = [CurvesRgbV1::default(); MAX];
    let (nl, ne, nc) = capture(doc, &mut l, &mut e, &mut c, check)?;
    let job = PreparedCurvesJob::new(
        &l[..nl],
        &e[..ne],
        &c[..nc],
        a.clone().with_byte_limit(bytes),
        check,
    )?;
    check()?;
    Ok(a.with_curves(Arc::new(job)))
}
pub fn with_document_programs<T>(
    doc: &Document,
    check: &dyn Fn() -> Result<()>,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    crate::direct::require_document(doc)?;
    if let Some(a) = crate::spatial::current()
        && a.prepared_curves().is_some()
    {
        validate_doc(doc, &a, check)?;
        return work();
    }
    if !active(doc) {
        return super::programs::with_legacy_programs(doc, check, work);
    }
    let a = crate::spatial::current().ok_or_else(super::programs::unprepared_curves)?;
    let a = prepare_document_programs(doc, a, check)?;
    crate::spatial::with_workspace(a, work)
}

#[cfg(test)]
#[path = "curves_tests.rs"]
mod tests;
