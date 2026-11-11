//! Admitted background preparation; the UI never captures/inverts A8 fields.
use super::*;
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_tools::LayerMaskEdit;
impl Editor {
    pub(crate) fn layer_mask_worker_available(&self) -> bool {
        self.editing.is_some()
    }
    pub(crate) fn layer_mask_capture(
        &self,
        operation: paintcraft_commands::LayerMaskOp,
    ) -> LayerMaskEdit {
        let ticket = self.session.ticket();
        LayerMaskEdit {
            owner: self.session.owner_id(),
            state_id: ticket.state_id,
            publication_epoch: ticket.publication_epoch,
            operation,
        }
    }
    pub(crate) fn validate_layer_mask_edit(&self, edit: &LayerMaskEdit) -> Result<()> {
        if self.editing.is_none() {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Layer masks require the editing worker",
            ));
        }
        let ticket = self.session.ticket();
        if !edit.matches_base(
            self.session.owner_id(),
            ticket.state_id,
            ticket.publication_epoch,
        ) || !edit
            .operation
            .stable_uuid_ref()
            .is_some_and(|id| self.target == Some(id) && self.document().layer(&id).is_some())
            || self.parameter_edit_open()
            || self.has_image_paste()
            || self.has_floating_selection()
            || self.gesture_in_progress()
            || self.is_editing_text()
            || self.is_constructing_shape()
            || self.failed_text_retained()
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Layer mask source or private input changed",
            ));
        }
        Ok(())
    }
    pub(super) fn execute_layer_mask(&mut self, edit: LayerMaskEdit) {
        let result = self.validate_layer_mask_edit(&edit).and_then(|()| {
            edit.operation
                .to_value()
                .and_then(|op| strokes::captured_batch(self.session.ticket(), vec![op]))
        });
        match result {
            // start_edit captures Preparation and submitted admission; all A8
            // inventory/capture/invert and persistent validation run in its task.
            Ok(batch) => self.start_edit(batch, "Edit", None),
            Err(error) => self.status = format!("Edit failed: {error}"),
        }
    }
}
