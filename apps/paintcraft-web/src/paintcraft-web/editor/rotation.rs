//! Captured custom rotation uses the existing atomic session and its admission.
use super::{Editor, MAX_PIXELS};
use paintcraft_core::types::{Error, ErrorCode, Result, rotation::Rotation};
use paintcraft_raster::Selection;
use paintcraft_tools::{RotateEdit, commands};

impl Editor {
    fn validate_rotate_edit(&self, edit: &RotateEdit) -> Result<()> {
        let ticket = self.session.ticket();
        if edit.owner != self.session.owner_id()
            || edit.state_id != ticket.state_id
            || edit.publication_epoch != ticket.publication_epoch
            || edit.target != self.target
            || edit.selection != self.session.selection()
            || self.tools.floating_selection().is_some()
            || self.tools.is_editing_text()
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Rotation source, target or selection changed",
            ));
        }
        Ok(())
    }

    pub(super) fn apply_rotate(&mut self, edit: &RotateEdit) -> Result<()> {
        self.validate_rotate_edit(edit)?;
        let Some(source) = commands::rotate_source(self.session.document(), &edit.selection)?
        else {
            return Ok(());
        };
        let geometry = Rotation::new(source, edit.coefficients)?;
        if edit.selection == Selection::Inactive {
            let bounds = geometry.bounds(source)?;
            if bounds.width() as u64 * bounds.height() as u64 > MAX_PIXELS {
                return Err(Error::new(
                    ErrorCode::ResourceLimit,
                    "Browser canvas limit is 4 megapixels",
                ));
            }
        }
        let operations = commands::rotate_image(
            self.session.document(),
            edit.selection.clone(),
            edit.target,
            edit.coefficients,
            edit.clear,
        )?;
        if operations.is_empty() {
            return Ok(());
        }
        self.tools.cancel_gesture();
        self.commit(operations)
    }
}

#[cfg(test)]
mod tests;
