//! Capture once during compilation; replay consumes the exact admitted fields.
use super::*;
use paintcraft_commands::LayerMaskOp;
impl Stage<'_> {
    pub(in crate::execute) fn edit_layer_mask(&mut self, operation: &LayerMaskOp) -> Result<()> {
        let layer = self.resolve(operation.references())?;
        let old = self.doc.layers[&layer].mask.clone();
        if matches!(operation, LayerMaskOp::Remove { .. }) && old.is_none()
            || matches!(operation, LayerMaskOp::SetEnabled { enabled, .. } if old.as_ref().is_some_and(|m| m.enabled() == *enabled))
        {
            return Ok(());
        }
        if self.planning.is_none() {
            let fields = self.captured_masks(layer)?;
            return self.install_masks(layer, &fields);
        }
        if old.is_none()
            && matches!(
                operation,
                LayerMaskOp::Invert { .. } | LayerMaskOp::SetEnabled { .. }
            )
        {
            return Err(Error::new(
                ErrorCode::TargetInvalid,
                "Layer has no stored mask",
            ));
        }
        // Invalid source state is diagnosed before coverage inventory/admission.
        if matches!(operation, LayerMaskOp::FromSelection { .. })
            && matches!(self.selection, Selection::Inactive)
        {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "selection mask requires an active selection",
            ));
        }
        let mut counts = self.layer_mask_counts(layer)?;
        let mut fields = fields(self, 0)?;
        let remaining = Cell::new(self.coverage_work);
        let check = || {
            (self.check)()?;
            remaining.set(remaining.get().checked_sub(1).ok_or_else(limit)?);
            Ok(())
        };
        let limits = counts.limits(remaining.get());
        let mask = match operation {
            LayerMaskOp::FromSelection { enabled, .. } => {
                let (coverage, spent) = A8Mask::from_selection_measured(
                    &self.selection,
                    self.doc.layers[&layer].extent(),
                    &self.adm.coverage_budget(),
                    limits,
                    self.check,
                )?;
                remaining.set(remaining.get().checked_sub(spent).ok_or_else(limit)?);
                Some(LayerMask::new(*enabled, coverage))
            }
            LayerMaskOp::SetEnabled { enabled, .. } => Some(
                old.as_ref()
                    .expect("checked presence")
                    .with_enabled(*enabled),
            ),
            LayerMaskOp::Invert { .. } => {
                let old = old.as_ref().expect("checked presence");
                Some(old.with_coverage(old.coverage().inverted_checked(
                    &self.adm.coverage_budget(),
                    limits,
                    &check,
                )?))
            }
            LayerMaskOp::Remove { .. } => None,
        };
        let mask = match (&old, mask) {
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
        fields.layer = LayerSlot::Replace(mask);
        self.remember_masks(layer, fields)
    }
}
#[cfg(test)]
mod tests;
