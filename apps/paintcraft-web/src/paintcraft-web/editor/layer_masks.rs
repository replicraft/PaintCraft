//! Captured own-mask intent uses the existing atomic browser transaction.
//! Normal/Multiply/Screen own masks use the same transaction; Studio display
//! owns Source7/8 selection. No A8 field is copied or inverted in the UI.
use super::*;
use paintcraft_commands::{LayerMaskOp, LayerRef};
use paintcraft_tools::LayerMaskEdit;
impl Editor {
    pub(crate) fn layer_mask_capture(&self, operation: LayerMaskOp) -> LayerMaskEdit {
        let ticket = self.session.ticket();
        LayerMaskEdit {
            owner: self.session.owner_id(),
            state_id: ticket.state_id,
            publication_epoch: ticket.publication_epoch,
            operation,
        }
    }
    pub(crate) fn layer_mask_available(&self) -> bool {
        self.target
            .is_some_and(|id| self.session.document().layer(&id).is_some())
            && !self.layer_mask_private_input()
    }
    fn layer_mask_private_input(&self) -> bool {
        self.tools.is_editing_text()
            || self.tools.gesture_in_progress()
            || self.tools.is_constructing_shape()
            || self.tools.floating_selection().is_some()
            || self.floating_source.is_some()
            || self.pen_base.is_some()
    }
    pub(crate) fn layer_mask_operation(&self, edit: &LayerMaskEdit) -> Result<Value> {
        let ticket = self.session.ticket();
        if !edit.matches_base(
            self.session.owner_id(),
            ticket.state_id,
            ticket.publication_epoch,
        ) {
            return Err(stale());
        }
        let LayerRef::Id(layer) = edit.operation.references() else {
            return Err(Error::new(
                ErrorCode::InputSchema,
                "Captured layer masks require a layer UUID",
            ));
        };
        if self.target != Some(*layer) || self.session.document().layer(layer).is_none() {
            return Err(stale());
        }
        if self.layer_mask_private_input() {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Finish or cancel the private tool draft before editing layer masks",
            ));
        }
        edit.operation.to_value()
    }
}
fn stale() -> Error {
    Error::new(
        ErrorCode::ConflictStaleBase,
        "Layer mask source or target changed",
    )
}
#[cfg(test)]
mod tests;
