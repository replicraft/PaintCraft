//! Admitted mask captures survive the compiler pass; replay never reads selection.
use super::*;
use paintcraft_commands::{FilterRef, MaskOp};
use paintcraft_core::types::{
    filters::A8Input,
    orthogonal::OrthogonalTransform,
    resources::{AllocationClass, BudgetAccount},
};
use paintcraft_document::{FilterMask, FilterStack};
use paintcraft_raster::coverage_mask::{A8Mask, Band, Limits};
use std::{cell::Cell, mem::size_of, sync::Arc};

pub(super) struct Captured {
    operation: usize,
    layer: Uuid,
    fields: Arc<Fields>,
}
struct Fields {
    entries: Vec<(Uuid, Option<FilterMask>)>,
    _charge: BudgetAccount,
}
#[derive(Clone, Copy)]
pub(super) enum Mapping {
    Empty,
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
        if fields.entries.is_empty() {
            return Ok(());
        }
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
        let mut fields = fields(self, 1)?;
        let remaining = Cell::new(self.coverage_work);
        let check = || {
            (self.check)()?;
            remaining.set(remaining.get().checked_sub(1).ok_or_else(limit)?);
            Ok(())
        };
        let limits = Limits {
            work: self.coverage_work,
            ..Limits::default()
        };
        let mask = match operation {
            MaskOp::Set { coverage, enabled } => Some(FilterMask::new(
                *enabled,
                from_input(coverage, self, limits, &check)?,
            )),
            MaskOp::FromSelection { enabled } => Some(FilterMask::new(
                *enabled,
                A8Mask::from_selection(
                    &self.selection,
                    self.doc.layers[&layer].extent(),
                    &self.adm.coverage_budget(),
                    limits,
                    &check,
                )?,
            )),
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
        self.coverage_work = remaining.get();
        fields.entries.push((entry.id(), mask));
        self.remember_masks(layer, fields)
    }
    /// Freeze transformed fields before any layer geometry is overwritten.
    pub(super) fn map_filter_masks(&mut self, layer: Uuid, mapping: Mapping) -> Result<()> {
        // Filter-free documents retain the legacy transform admission path.
        if !self.doc.layers[&layer]
            .filters
            .iter()
            .any(|entry| entry.mask().is_some())
        {
            return Ok(());
        }
        if self.planning.is_none() {
            let fields = self.captured_masks(layer)?;
            return self.install_masks(layer, &fields);
        }
        let entries = &self.doc.layers[&layer].filters;
        let mut fields = fields(
            self,
            entries
                .iter()
                .filter(|entry| entry.mask().is_some())
                .count(),
        )?;
        let remaining = Cell::new(self.coverage_work);
        let check = || {
            (self.check)()?;
            remaining.set(remaining.get().checked_sub(1).ok_or_else(limit)?);
            Ok(())
        };
        for entry in entries.iter() {
            let Some(mask) = entry.mask() else {
                continue;
            };
            check()?;
            let limits = Limits {
                work: remaining.get(),
                ..Limits::default()
            };
            let coverage = match mapping {
                Mapping::Empty => A8Mask::from_bands(
                    Rect::from_origin_size(0, 0, 1, 1).unwrap(),
                    &[],
                    &self.adm.coverage_budget(),
                    limits,
                    &check,
                )?,
                Mapping::Crop(area) => {
                    mask.coverage()
                        .cropped_checked(area, remaining.get(), &check)?
                }
                Mapping::Orthogonal(size, transform) => mask.coverage().orthogonal_checked(
                    size,
                    transform,
                    &self.adm.coverage_budget(),
                    limits,
                    &check,
                )?,
                Mapping::Rebase { offset } => mask.coverage().translated_checked(
                    offset.0,
                    offset.1,
                    remaining.get(),
                    &check,
                )?,
            };
            fields
                .entries
                .push((entry.id(), Some(mask.with_coverage(coverage))));
        }
        self.coverage_work = remaining.get();
        self.remember_masks(layer, fields)
    }
}
