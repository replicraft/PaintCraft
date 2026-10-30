//! Disposable move feedback follows the same visible FIFO prefix as raster previews.
use super::*;
use paintcraft_core::types::Rect;
impl Editor {
    fn move_target_current(&self, layer: Uuid) -> bool {
        self.target == Some(layer)
            && self
                .document()
                .layers
                .get(&layer)
                .is_some_and(|layer| layer.visible)
    }
    /// Never skip a structural/file boundary to preview a later movement.
    pub(super) fn queued_move_preview(&self) -> Option<Preview> {
        if self.file_barrier.is_some() || self.has_inflight_edit() || self.text_placement_failed() {
            return None;
        }
        let action = match self.actions.front() {
            Some(background::Queued::Edit(queued)) => Some(&queued.action),
            Some(
                background::Queued::FileBarrier(_) | background::Queued::TextCaptureFailure(_),
            ) => None,
            None => self.outbox.pending().next(),
        }?;
        match action {
            Action::MoveSelection {
                layer,
                offset,
                clear,
                transparent_key,
            } if self.move_target_current(*layer) => Some(Preview::MoveSelection {
                layer: *layer,
                offset: *offset,
                clear: *clear,
                transparent_key: *transparent_key,
            }),
            _ => None,
        }
    }
    pub(super) fn move_preview_current(&self, preview: &Preview) -> bool {
        match preview {
            Preview::MoveSelection { layer, .. } => {
                self.move_target_current(*layer)
                    && (self.has_inflight_edit()
                        || self.queued_move_preview().as_ref() == Some(preview))
            }
            _ => true,
        }
    }
    pub(super) fn pending_move_selection(&self) -> Option<Selection> {
        let preview = self
            .publication_preview
            .as_ref()
            .filter(|preview| self.move_preview_current(preview))
            .cloned()
            .or_else(|| self.queued_move_preview())?;
        let Preview::MoveSelection { offset, .. } = preview else {
            return None;
        };
        let selection = self.selection();
        let canvas = self.document().canvas_rect();
        // Translation does not need a tight bounding box: clipping to the finite
        // canvas first yields exactly the selected source samples. No scan occurs.
        // If a bounded edge proof is unavailable, omit derived feedback rather
        // than showing the old position or pretending the selection is empty.
        Some(
            self.moved_selection_field(&selection, canvas, offset)
                .unwrap_or(Selection::Inactive),
        )
    }
    pub(super) fn moved_selection_field(
        &self,
        selection: &Selection,
        bounds: Rect,
        offset: (i64, i64),
    ) -> Result<Selection> {
        let canvas = self.document().canvas_rect();
        Ok(match selection {
            Selection::Rect(rect) => rect
                .intersect(&bounds)
                .and_then(|area| area.translated(offset.0, offset.1))
                .and_then(|area| area.intersect(&canvas))
                .map_or(Selection::Empty, Selection::Rect),
            Selection::Mask(mask) => {
                let Some(source) = mask.clipped(bounds)? else {
                    return Ok(Selection::Empty);
                };
                let moved = match source.translated(offset.0, offset.1) {
                    Ok(moved) => moved,
                    Err(error) => {
                        // At coordinate-policy edges, zero margins in a mask's
                        // finite domain can overflow while every selected sample
                        // remains valid. Prove tighter bounds before retrying.
                        let Some(tight) = source.bounds_checked(
                            bounds,
                            paintcraft_raster::selection::BOUNDS_CACHE_WORK,
                            &|| Ok(()),
                        )?
                        else {
                            return Ok(Selection::Empty);
                        };
                        let Some(source) = source.clipped(tight)? else {
                            return Ok(Selection::Empty);
                        };
                        source.translated(offset.0, offset.1).map_err(|_| error)?
                    }
                };
                moved
                    .clipped(canvas)?
                    .map_or(Selection::Empty, Selection::Mask)
            }
            Selection::Inactive => Selection::Inactive,
            Selection::Empty => Selection::Empty,
        })
    }
}
#[cfg(test)]
mod tests;
