//! Admitted mask captures survive the compiler pass; replay never reads selection.
use super::*;
use paintcraft_commands::{FilterRef, MaskOp};
use paintcraft_core::types::{
    filters::A8Input,
    orthogonal::OrthogonalTransform,
    resources::{AllocationClass, BudgetAccount},
};
use paintcraft_document::{FilterMask, FilterStack, LayerMask};
mod layer;
mod mapping;
pub(super) use mapping::Counts;
use paintcraft_raster::coverage_mask::{A8Mask, Band, Limits};
use std::{cell::Cell, mem::size_of, sync::Arc};

pub(super) struct Captured {
    operation: usize,
    layer: Uuid,
    fields: Arc<Fields>,
}
enum LayerSlot {
    Keep,
    Replace(Option<LayerMask>),
}
struct Fields {
    layer: LayerSlot,
    entries: Vec<(Uuid, Option<FilterMask>)>,
    _charge: BudgetAccount,
}
#[derive(Clone, Copy)]
pub(super) enum Mapping {
    Empty,
    StretchSkew(paintcraft_core::types::stretch_skew::StretchSkew, Rect),
    Crop(Rect),
    Orthogonal((u32, u32), OrthogonalTransform),
    Rebase { offset: (i64, i64) },
}
fn limit() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Filter mask staging limit")
}
fn fields(stage: &Stage<'_>, count: usize) -> Result<Fields> {
    let mut charge = stage
        .adm
        .coverage_budget()
        .account(AllocationClass::CoverageMasks);
    let quote = |count: usize| {
        count
            .checked_mul(size_of::<(Uuid, Option<FilterMask>)>())
            .and_then(|n| n.checked_add(size_of::<Fields>() + 2 * size_of::<usize>()))
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(limit)
    };
    charge.resize(quote(count)?)?;
    let mut entries = Vec::new();
    entries.try_reserve_exact(count).map_err(|_| limit())?;
    if let Err(error) = charge.resize(quote(entries.capacity())?) {
        drop(entries);
        return Err(error);
    }
    Ok(Fields {
        layer: LayerSlot::Keep,
        entries,
        _charge: charge,
    })
}
fn from_input(
    input: &A8Input,
    stage: &Stage<'_>,
    limits: Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<A8Mask> {
    let budget = stage.adm.coverage_budget();
    let mut charge = budget.account(AllocationClass::CoverageMasks);
    let quote = |n: usize| {
        n.checked_mul(size_of::<Band<'_>>())
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(limit)
    };
    charge.resize(quote(input.band_count())?)?;
    let mut bands = Vec::new();
    bands
        .try_reserve_exact(input.band_count())
        .map_err(|_| limit())?;
    if let Err(error) = charge.resize(quote(bands.capacity())?) {
        drop(bands);
        return Err(error);
    }
    for band in input.bands() {
        check()?;
        bands.push(band);
    }
    A8Mask::from_bands(input.domain(), &bands, &budget, limits, check)
}
impl Stage<'_> {
    fn captured_masks(&mut self, layer: Uuid) -> Result<Arc<Fields>> {
        let captured = self
            .filter_masks
            .values
            .get(self.mask_replay)
            .filter(|c| c.operation == self.operation && c.layer == layer)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::InvariantViolation,
                    "Compiled filter masks missing",
                )
            })?;
        self.mask_replay += 1;
        Ok(captured.fields.clone())
    }
    fn install_masks(&mut self, layer: Uuid, fields: &Fields) -> Result<()> {
        if !fields.entries.is_empty() {
            let mut instances = self.filter_instances(layer, false)?;
            for (id, mask) in &fields.entries {
                (self.check)()?;
                let entry = instances
                    .iter_mut()
                    .find(|entry| entry.id() == *id)
                    .ok_or_else(|| {
                        Error::new(ErrorCode::InvariantViolation, "Captured filter missing")
                    })?;
                *entry = entry.with_mask(mask.clone());
            }
            self.doc
                .layers
                .get_mut(&layer)
                .expect("resolved layer")
                .filters = FilterStack::new(instances)?;
        }
        if let LayerSlot::Replace(mask) = &fields.layer {
            (self.check)()?;
            self.doc
                .layers
                .get_mut(&layer)
                .expect("resolved layer")
                .mask = mask.clone();
        }
        Ok(())
    }
    fn remember_masks(&mut self, layer: Uuid, fields: Fields) -> Result<()> {
        let fields = Arc::new(fields);
        self.filter_masks.push(Captured {
            operation: self.operation,
            layer,
            fields: fields.clone(),
        })?;
        self.install_masks(layer, &fields)
    }
    pub(super) fn edit_filter_mask(
        &mut self,
        layer: Uuid,
        reference: &FilterRef,
        operation: &MaskOp,
    ) -> Result<()> {
        let position = self.filter_position(layer, reference)?;
        let entry = &self.doc.layers[&layer].filters.as_slice()[position];
        if matches!(operation, MaskOp::Remove) && entry.mask().is_none()
            || matches!(operation, MaskOp::SetEnabled { enabled } if entry.mask().is_some_and(|mask| mask.enabled() == *enabled))
        {
            return Ok(());
        }
        if self.planning.is_none() {
            let fields = self.captured_masks(layer)?;
            return self.install_masks(layer, &fields);
        }
        let entry = self.doc.layers[&layer].filters.as_slice()[position].clone();
        let mut counts = self.filter_mask_counts(layer, entry.id())?;
        let mut fields = fields(self, 1)?;
        let remaining = Cell::new(self.coverage_work);
        let check = || {
            (self.check)()?;
            remaining.set(remaining.get().checked_sub(1).ok_or_else(limit)?);
            Ok(())
        };
        let limits = counts.limits(self.coverage_work);
        let mask = match operation {
            MaskOp::Set { coverage, enabled } => Some(FilterMask::new(
                *enabled,
                from_input(coverage, self, limits, &check)?,
            )),
            MaskOp::FromSelection { enabled } => {
                let (coverage, spent) = A8Mask::from_selection_measured(
                    &self.selection,
                    self.doc.layers[&layer].extent(),
                    &self.adm.coverage_budget(),
                    limits,
                    self.check,
                )?;
                remaining.set(remaining.get().checked_sub(spent).ok_or_else(limit)?);
                Some(FilterMask::new(*enabled, coverage))
            }
            MaskOp::SetEnabled { enabled } => Some(
                entry
                    .mask()
                    .ok_or_else(|| {
                        Error::new(ErrorCode::TargetInvalid, "Filter has no stored mask")
                    })?
                    .with_enabled(*enabled),
            ),
            MaskOp::Remove => None,
        };
        // Reuse the original on exact semantic no-ops, including noncanonical DTO input.
        let mask = match (entry.mask(), mask) {
            (Some(old), Some(new))
                if old.enabled() == new.enabled()
                    && old.coverage().same_coverage_checked(
                        new.coverage(),
                        remaining.get(),
                        &check,
                    )? =>
            {
                Some(old.clone())
            }
            (_, new) => new,
        };
        if let Some(mask) = &mask {
            counts.include(mask.coverage())?;
        }
        self.coverage_work = remaining.get();
        fields.entries.push((entry.id(), mask));
        self.remember_masks(layer, fields)
    }
    /// Freeze BOTH mask families before geometry changes; replay only installs.
    pub(super) fn map_filter_masks(
        &mut self,
        layer: Uuid,
        mapping: Mapping,
        counts: &mut Counts,
    ) -> Result<()> {
        let source = &self.doc.layers[&layer];
        if source.mask.is_none() && !source.filters.iter().any(|e| e.mask().is_some()) {
            return Ok(());
        }
        if self.planning.is_none() {
            let fields = self.captured_masks(layer)?;
            return self.install_masks(layer, &fields);
        }
        let mut fields = fields(
            self,
            source.filters.iter().filter(|e| e.mask().is_some()).count(),
        )?;
        let remaining = Cell::new(self.coverage_work);
        let check = || {
            (self.check)()?;
            remaining.set(remaining.get().checked_sub(1).ok_or_else(limit)?);
            Ok(())
        };
        for entry in source.filters.iter() {
            let Some(mask) = entry.mask() else { continue };
            check()?;
            let coverage = mapping::coverage(
                mask.coverage(),
                self,
                mapping,
                counts.limits(remaining.get()),
                &check,
            )?;
            counts.include(&coverage)?;
            fields
                .entries
                .push((entry.id(), Some(mask.with_coverage(coverage))));
        }
        if let Some(mask) = &source.mask {
            check()?;
            let coverage = mapping::coverage(
                mask.coverage(),
                self,
                mapping,
                counts.limits(remaining.get()),
                &check,
            )?;
            counts.include(&coverage)?;
            fields.layer = LayerSlot::Replace(Some(mask.with_coverage(coverage)));
        }
        self.coverage_work = remaining.get();
        self.remember_masks(layer, fields)
    }
}
