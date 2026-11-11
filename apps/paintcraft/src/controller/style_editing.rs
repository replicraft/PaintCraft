//! Captured whole-layer style edits use the existing admitted FIFO and renderer.
use super::*;
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_tools::StyleEdit;
#[path = "style_editing/submission.rs"]
mod submission;
pub(crate) use submission::Feedback;
#[derive(Debug, Default)]
pub(super) struct State {
    pub(super) open: bool,
    preview: Option<StyleEdit>,
    pub(super) submission: submission::Tracker,
}
impl Editor {
    pub(crate) fn style_worker_available(&self) -> bool {
        self.editing.is_some()
    }
    pub(super) fn parameter_edit_open(&self) -> bool {
        self.filter_edit_open || self.style_editing.open
    }
    pub(crate) fn style_capture(&self, operation: paintcraft_commands::StyleOp) -> StyleEdit {
        let ticket = self.session.ticket();
        StyleEdit {
            owner: self.session.owner_id(),
            state_id: ticket.state_id,
            publication_epoch: ticket.publication_epoch,
            operation,
        }
    }
    pub(crate) fn style_base_current(&self, edit: &StyleEdit) -> bool {
        let ticket = self.session.ticket();
        edit.matches_base(
            self.session.owner_id(),
            ticket.state_id,
            ticket.publication_epoch,
        )
    }
    pub(crate) fn begin_style_edit(&mut self) -> Result<()> {
        if !self.style_worker_available() {
            return Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "Layer styles require the editing worker",
            ));
        }
        if self.filter_edit_open
            || self.has_image_paste()
            || self.has_floating_selection()
            || self.is_constructing_shape()
            || self.gesture_in_progress()
            || self.failed_text_retained()
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Finish the current placement before editing styles",
            ));
        }
        self.cancel_gesture();
        self.commit_text();
        Ok(())
    }
    pub(crate) fn set_style_edit_open(&mut self, open: bool) {
        if self.style_editing.open != open {
            self.style_editing.open = open;
            self.preview_lineage = Uuid::new_v4();
            self.preview_queue_revision.advance();
        }
        if !open {
            self.set_style_preview(None);
        }
    }
    pub(crate) fn set_style_preview(&mut self, preview: Option<StyleEdit>) {
        if self.style_editing.preview != preview {
            self.style_editing.preview = preview;
            self.preview_queue_revision.advance();
        }
    }
    pub(super) fn style_preview_current(&self) -> Option<Preview> {
        self.style_editing
            .preview
            .as_ref()
            .filter(|edit| self.validate_style_edit(edit).is_ok())
            .cloned()
            .map(Preview::Style)
    }
    pub(super) fn validate_style_edit(&self, edit: &StyleEdit) -> Result<()> {
        let valid = self.style_worker_available()
            && self.style_base_current(edit)
            && !self.filter_edit_open
            && !self.has_image_paste()
            && !self.has_floating_selection()
            && !self.gesture_in_progress()
            && !self.is_editing_text()
            && !self.is_constructing_shape()
            && !self.failed_text_retained()
            && edit
                .operation
                .stable_uuid_refs()
                .is_some_and(|(layer, style)| {
                    self.target == Some(layer)
                        && self.document().layer(&layer).is_some_and(|l| {
                            style.is_none_or(|id| l.styles.iter().any(|s| s.id() == id))
                        })
                });
        if valid {
            Ok(())
        } else {
            Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Style source or stable reference changed",
            ))
        }
    }
    pub(super) fn execute_style(&mut self, edit: StyleEdit) {
        let attempt = self.start_style_attempt(&edit);
        if self.validate_style_edit(&edit).is_err() {
            self.finish_style_attempt(attempt, false);
            self.status = "Style not applied: its document changed".into();
            self.style_editing.submission.executing = None;
            return;
        }
        let batch = edit
            .operation
            .to_value()
            .and_then(|op| strokes::captured_batch(self.session.ticket(), vec![op]));
        match batch {
            Ok(batch) => {
                let receipt = self.commit_batch(batch, "Style applied", None);
                if self.editing.is_none() {
                    self.finish_style_attempt(attempt, receipt.is_some());
                }
            }
            Err(error) => {
                self.finish_style_attempt(attempt, false);
                self.status = format!("Style not applied: {error}");
            }
        }
        self.style_editing.submission.executing = None;
    }
}
