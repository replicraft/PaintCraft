//! Source8-only canonical metadata capture. Existing capture/sort stays exact.
use super::{
    DocumentProgramsQuote, PreparedCurvesJob, PreparedLevelsJob, PreparedPointJob,
    WorkspaceAdmission,
};
use paintcraft_core::types::{
    Error, ErrorCode, LayerBlendMode, Result,
    filters::{CurvesRgbV1, Descriptor, ExposureSrgbV1, LevelsRgbV1},
};
use paintcraft_document::{Document, Layer};
use std::sync::Arc;
mod ledger;
use ledger::*;
const MAX: usize = PreparedPointJob::MAX_PROGRAMS;
pub(crate) const CAPTURE_BYTES: u64 = std::mem::size_of::<Keys>() as u64;
fn overflow() -> Error {
    Error::new(
        ErrorCode::ResourceLimit,
        "Blend metadata allowance overflow",
    )
}
fn missing() -> Error {
    Error::new(
        ErrorCode::CapabilityUnsupported,
        "Blend prepared keys or Noise owner missing",
    )
}
#[derive(Debug)]
pub(crate) struct Keys {
    levels: [LevelsRgbV1; MAX],
    exposures: [ExposureSrgbV1; MAX],
    curves: [CurvesRgbV1; MAX],
    lengths: [usize; 3],
    roots: usize,
    filters: u64,
    occurrences: [u64; 3],
    pub capture_work: u64,
    ambient: Option<[usize; 3]>,
    ambient_distinct: bool,
}
fn shape(doc: &Document) -> Result<()> {
    if doc.roots.len() > paintcraft_document::LAYER_COUNT_MAX || doc.roots.len() != doc.layers.len()
    {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Blend metadata requires bounded flat roots",
        ));
    }
    Ok(())
}
fn descriptor_counts(layer: &Layer, noise: bool) -> Result<[u64; 3]> {
    let mut counts = [0; 3];
    for filter in layer.filters.iter() {
        if matches!(filter.descriptor(), Descriptor::AddNoiseRgbV1(_)) && !noise {
            return Err(missing());
        }
        if filter.enabled() {
            let slot = match filter.descriptor() {
                Descriptor::LevelsRgbV1(_) => Some(0),
                Descriptor::ExposureSrgbV1(_) => Some(1),
                Descriptor::CurvesRgbV1(_) => Some(2),
                _ => None,
            };
            if let Some(i) = slot {
                counts[i] += 1;
            }
        }
    }
    Ok(counts)
}
fn charge(total: &mut u64, amount: u64, spend: &dyn Fn(u64) -> Result<()>) -> Result<()> {
    spend(amount)?;
    *total = add(*total, amount)?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn insert<T: Copy, const WIDTH: usize>(
    values: &mut [T; MAX],
    count: &mut usize,
    all: &mut usize,
    value: T,
    key: impl Fn(T) -> [u8; WIDTH],
    total: &mut u64,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    charge(
        total,
        add(
            eq(*count + 1, WIDTH as u64)?,
            mul(*count as u64, WIDTH as u64)?,
        )?,
        spend,
    )?;
    let value_key = key(value);
    let mut at = 0;
    while at < *count && key(values[at]) < value_key {
        at += 1;
    }
    if at < *count && key(values[at]) == value_key {
        return Ok(());
    }
    if *all == MAX {
        return Err(overflow());
    }
    values.copy_within(at..*count, at + 1);
    values[at] = value;
    *count += 1;
    *all += 1;
    Ok(())
}
/// Explicit metadata ownership policy; unbound forecasting has no admission.
#[derive(Clone, Copy)]
enum OwnerPolicy<'a> {
    Bound {
        explicit: &'a WorkspaceAdmission,
        ambient: Option<&'a WorkspaceAdmission>,
    },
    UnboundMetadata,
}
impl Keys {
    pub(crate) fn capture(
        doc: &Document,
        explicit: &WorkspaceAdmission,
        ambient: Option<&WorkspaceAdmission>,
        noise: bool,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<Self> {
        Self::capture_with(
            doc,
            OwnerPolicy::Bound { explicit, ambient },
            noise,
            check,
            spend,
        )
        .map(|(keys, _)| keys)
    }
    /// Stored Noise includes disabled/default descriptors; no Noise grant implied.
    pub(crate) fn capture_unbound(
        doc: &Document,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<(Self, bool)> {
        Self::capture_with(doc, OwnerPolicy::UnboundMetadata, true, check, spend)
    }
    fn capture_with(
        doc: &Document,
        policy: OwnerPolicy<'_>,
        noise: bool,
        check: &dyn Fn() -> Result<()>,
        spend: &dyn Fn(u64) -> Result<()>,
    ) -> Result<(Self, bool)> {
        check()?;
        shape(doc)?;
        let (ambient_lengths, ambient_distinct) = match policy {
            OwnerPolicy::Bound { explicit, ambient } => (
                ambient.filter(|a| a.has_prepared_programs()).map(lengths),
                ambient.is_some_and(|a| a.has_prepared_programs() && !same_owner(explicit, a)),
            ),
            OwnerPolicy::UnboundMetadata => (None, false),
        };
        let mut keys = Self {
            levels: [LevelsRgbV1::default(); MAX],
            exposures: [ExposureSrgbV1::default(); MAX],
            curves: [CurvesRgbV1::default(); MAX],
            lengths: [0; 3],
            roots: doc.roots.len(),
            filters: 0,
            occurrences: [0; 3],
            capture_work: 0,
            ambient: ambient_lengths,
            ambient_distinct,
        };
        charge(&mut keys.capture_work, entry(doc.roots.len())?, spend)?;
        let owners = match policy {
            OwnerPolicy::Bound { explicit, ambient } => owners(explicit, ambient),
            OwnerPolicy::UnboundMetadata => [None, None],
        };
        let mut stored_noise = false;
        let mut all = 0;
        let mut non_normal = false;
        for id in &doc.roots {
            check()?;
            let layer =
                doc.layers.get(id).filter(|l| l.id == *id).ok_or_else(|| {
                    Error::new(ErrorCode::InputSchema, "Blend root identity differs")
                })?;
            non_normal |= layer.blend_mode != LayerBlendMode::Normal;
            let filters = layer.filters.len() as u64;
            if filters > 256 {
                return Err(overflow());
            }
            // Counts and canonical insertion share this single descriptor walk.
            charge(&mut keys.capture_work, mul(FILTER, filters)?, spend)?;
            let mut occurrences = [0u64; 3];
            for filter in layer.filters.iter() {
                check()?;
                if matches!(filter.descriptor(), Descriptor::AddNoiseRgbV1(_)) {
                    stored_noise = true;
                    if !noise {
                        return Err(missing());
                    }
                }
                if !filter.enabled() {
                    continue;
                }
                match filter.descriptor() {
                    Descriptor::LevelsRgbV1(p) => {
                        occurrences[0] += 1;
                        insert(
                            &mut keys.levels,
                            &mut keys.lengths[0],
                            &mut all,
                            p,
                            LevelsRgbV1::semantic_bytes,
                            &mut keys.capture_work,
                            spend,
                        )?;
                    }
                    Descriptor::ExposureSrgbV1(p) => {
                        occurrences[1] += 1;
                        insert(
                            &mut keys.exposures,
                            &mut keys.lengths[1],
                            &mut all,
                            p,
                            ExposureSrgbV1::semantic_bytes,
                            &mut keys.capture_work,
                            spend,
                        )?;
                    }
                    Descriptor::CurvesRgbV1(p) => {
                        occurrences[2] += 1;
                        insert(
                            &mut keys.curves,
                            &mut keys.lengths[2],
                            &mut all,
                            p,
                            CurvesRgbV1::semantic_bytes,
                            &mut keys.capture_work,
                            spend,
                        )?;
                    }
                    _ => {}
                }
            }
            keys.filters = add(keys.filters, filters)?;
            for (sum, n) in keys.occurrences.iter_mut().zip(occurrences) {
                *sum = add(*sum, n)?;
            }
            for owner in owners.into_iter().flatten() {
                charge(
                    &mut keys.capture_work,
                    validation(filters, occurrences, lengths(owner))?,
                    spend,
                )?;
                super::validate_layer_programs(layer, owner, check)?;
            }
        }
        if !non_normal {
            return Err(missing());
        }
        check()?;
        Ok((keys, stored_noise))
    }
    pub(crate) fn quote_metadata(&self) -> Result<u64> {
        quote_metadata(self.lengths)
    }
    pub(crate) fn constructor_metadata(&self) -> Result<u64> {
        constructor_metadata(self.lengths)
    }
    pub(crate) fn validation_work(&self, explicit: &WorkspaceAdmission) -> Result<u64> {
        self.validation_work_for(
            if explicit.has_prepared_programs() {
                lengths(explicit)
            } else {
                self.lengths
            },
            explicit.has_prepared_programs(),
        )
    }
    pub(crate) fn unbound_validation_work(&self) -> Result<u64> {
        self.validation_work_for(self.lengths, false)
    }
    fn validation_work_for(&self, future: [usize; 3], bound: bool) -> Result<u64> {
        let mut work = add(entry(self.roots)?, mul(FILTER, self.filters)?)?;
        if bound || future.iter().any(|n| *n != 0) {
            work = add(work, validation(self.filters, self.occurrences, future)?)?;
        }
        if self.ambient_distinct
            && let Some(ambient) = self.ambient
        {
            work = add(work, validation(self.filters, self.occurrences, ambient)?)?;
        }
        Ok(work)
    }
    /// Caller prepays quote_metadata before invoking existing bounded helpers.
    pub(crate) fn quote(&self) -> Result<DocumentProgramsQuote> {
        let [l, e, c] = self.lengths;
        if l + e + c == 0 {
            return Ok(DocumentProgramsQuote::default());
        }
        let (work, peak, retained) = if c != 0 {
            let q = super::curves_job::quote(
                &self.levels[..l],
                &self.exposures[..e],
                &self.curves[..c],
            )?;
            (q.work, q.peak, q.retained)
        } else if e != 0 {
            let q = super::point_job::quote(&self.levels[..l], &self.exposures[..e])?;
            (q.compile_work, q.peak_bytes, q.retained_bytes)
        } else {
            let q = super::quote_levels_job(&self.levels[..l])?;
            (
                q.compile_work,
                q.peak_bytes,
                add(q.index_bytes, mul(l as u64, super::LEVELS_TABLE_BYTES)?)?,
            )
        };
        Ok(DocumentProgramsQuote {
            levels_programs: l,
            exposure_programs: e,
            unique_programs: l + e + c,
            compile_work: work,
            preparation_peak_bytes: add(CAPTURE_BYTES, peak)?,
            retained_bytes: retained,
        })
    }
    /// Metadata and aggregate peak were admitted before these existing builders.
    pub(crate) fn prepare(
        &self,
        a: WorkspaceAdmission,
        check: &dyn Fn() -> Result<()>,
    ) -> Result<WorkspaceAdmission> {
        let [l, e, c] = self.lengths;
        if a.has_prepared_programs() || l + e + c == 0 {
            return Ok(a);
        }
        let bytes = a
            .bytes_limit()
            .checked_sub(CAPTURE_BYTES)
            .ok_or_else(overflow)?;
        let child = a.clone().with_byte_limit(bytes);
        if c != 0 {
            Ok(a.with_curves(Arc::new(PreparedCurvesJob::new(
                &self.levels[..l],
                &self.exposures[..e],
                &self.curves[..c],
                child,
                check,
            )?)))
        } else if e != 0 {
            Ok(a.with_programs(Arc::new(PreparedPointJob::new(
                &self.levels[..l],
                &self.exposures[..e],
                child,
                check,
            )?)))
        } else {
            Ok(a.with_levels(Arc::new(PreparedLevelsJob::new(
                &self.levels[..l],
                child,
                check,
            )?)))
        }
    }
}
/// Fresh complete-key validation only. No capture/sort/compile or cached grant.
pub(crate) fn validate(
    doc: &Document,
    explicit: &WorkspaceAdmission,
    ambient: Option<&WorkspaceAdmission>,
    noise: bool,
    check: &dyn Fn() -> Result<()>,
    spend: &dyn Fn(u64) -> Result<()>,
) -> Result<()> {
    check()?;
    shape(doc)?;
    spend(entry(doc.roots.len())?)?;
    let owners = owners(explicit, ambient);
    let mut non_normal = false;
    for id in &doc.roots {
        check()?;
        let layer = doc
            .layers
            .get(id)
            .filter(|l| l.id == *id)
            .ok_or_else(|| Error::new(ErrorCode::InputSchema, "Blend root identity differs"))?;
        non_normal |= layer.blend_mode != LayerBlendMode::Normal;
        let filters = layer.filters.len() as u64;
        if filters > 256 {
            return Err(overflow());
        }
        spend(mul(FILTER, filters)?)?;
        let occurrences = descriptor_counts(layer, noise)?;
        if occurrences.iter().any(|n| *n != 0) && !explicit.has_prepared_programs() {
            return Err(missing());
        }
        for owner in owners.into_iter().flatten() {
            spend(validation(filters, occurrences, lengths(owner))?)?;
            super::validate_layer_programs(layer, owner, check)?;
        }
    }
    if !non_normal {
        return Err(missing());
    }
    check()
}
#[cfg(test)]
mod tests;

/// Conservative union of actual distinct complete payload owners. Identical
/// numerical keys are never evidence of shared storage. Existing constructors
/// create each complete job with its own tables/index; cloned top owner pointers
/// are the only shared complete payload represented by WorkspaceAdmission.
pub(crate) fn bound_retained_bytes(
    explicit: &WorkspaceAdmission,
    ambient: Option<&WorkspaceAdmission>,
) -> Result<u64> {
    owners(explicit, ambient)
        .into_iter()
        .flatten()
        .try_fold(0, |sum, owner| add(sum, owner.prepared_retained_bytes()))
}
