//! Captured rotate intentions revalidate both at queue admission and dispatch.
use super::*;
impl Editor {
    pub(in crate::controller) fn validate_rotate_edit(
        &self,
        edit: &paintcraft_tools::RotateEdit,
    ) -> Result<()> {
        let ticket = self.session.ticket();
        if edit.owner != self.session.owner_id()
            || edit.state_id != ticket.state_id
            || edit.publication_epoch != ticket.publication_epoch
            || edit.target != self.target
            || edit.selection != self.selection()
            || self.has_image_paste()
            || self.has_floating_selection()
            || !self.text_save_ready()
        {
            return Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::ConflictStaleBase,
                "Rotation source, target or selection changed",
            ));
        }
        Ok(())
    }
    pub(in crate::controller) fn rotate_image(&mut self, edit: paintcraft_tools::RotateEdit) {
        if let Err(error) = self.validate_rotate_edit(&edit) {
            self.status = error.to_string();
            return;
        }
        match paintcraft_tools::commands::rotate_image(
            self.document(),
            edit.selection,
            edit.target,
            edit.coefficients,
            edit.clear,
        ) {
            Ok(ops) if !ops.is_empty() => {
                self.cancel_gesture();
                self.draw(ops, "Rotate image");
            }
            Ok(_) => {}
            Err(error) => self.status = error.to_string(),
        }
    }
}
#[cfg(test)]
#[path = "rotation/tests.rs"]
mod tests;
