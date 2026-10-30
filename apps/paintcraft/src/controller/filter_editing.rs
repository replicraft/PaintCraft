//! Captured filter intentions use the same FIFO and publication path as paint.
use super::*;
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_tools::FilterEdit;

impl Editor {
    pub(crate) fn filter_capture(&self, operation: paintcraft_commands::FilterOp) -> FilterEdit {
        let ticket = self.session.ticket();
        FilterEdit {
            owner: self.session.owner_id(),
            state_id: ticket.state_id,
            publication_epoch: ticket.publication_epoch,
            operation,
        }
    }
    pub(crate) fn filter_base_current(&self, edit: &FilterEdit) -> bool {
        let ticket = self.session.ticket();
        edit.matches_base(
            self.session.owner_id(),
            ticket.state_id,
            ticket.publication_epoch,
        )
    }
    pub(crate) fn set_filter_edit_open(&mut self, open: bool) {
        if self.filter_edit_open != open {
            self.filter_edit_open = open;
            self.preview_lineage = Uuid::new_v4();
            self.preview_queue_revision.advance();
        }
        if !open {
            self.set_filter_preview(None);
        }
    }
    pub(crate) fn set_filter_preview(&mut self, edit: Option<FilterEdit>) {
        if self.filter_preview != edit {
            self.filter_preview = edit;
            self.preview_queue_revision.advance();
        }
    }
    pub(super) fn filter_preview_current(&self) -> Option<Preview> {
        self.filter_preview
            .as_ref()
            .filter(|edit| self.filter_base_current(edit))
            .cloned()
            .map(Preview::Filter)
    }
    pub(super) fn execute_filter(&mut self, edit: FilterEdit) {
        let attempt = self.start_filter_attempt(&edit);
        // Check here, after accepted prefix actions, rather than at UI enqueue.
        if self.validate_filter_edit(&edit).is_err() {
            self.finish_filter_attempt(attempt, false);
            self.status = "Filter not applied: its document changed".into();
            self.filter_submission.executing = None;
            return;
        }
        let batch = edit
            .operation
            .to_value()
            .and_then(|op| strokes::captured_batch(self.session.ticket(), vec![op]));
        match batch {
            Ok(batch) => {
                let receipt = self.commit_batch(batch, "Filter applied", None);
                if self.editing.is_none() {
                    self.finish_filter_attempt(attempt, receipt.is_some());
                }
            }
            Err(error) => {
                self.finish_filter_attempt(attempt, false);
                self.status = format!("Filter not applied: {error}");
            }
        }
        self.filter_submission.executing = None;
    }
    pub(crate) fn begin_filter_edit(&mut self) -> Result<()> {
        if self.has_image_paste() || self.has_floating_selection() || self.is_constructing_shape() {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Finish the current placement before editing filters",
            ));
        }
        self.cancel_gesture();
        self.commit_text();
        Ok(())
    }
}

impl Editor {
    pub(super) fn validate_filter_edit(&self, edit: &FilterEdit) -> Result<()> {
        let valid = self.filter_base_current(edit)
            && edit
                .operation
                .stable_uuid_refs()
                .is_some_and(|(id, filter)| {
                    self.document().layer(&id).is_some_and(|layer| {
                        filter.is_none_or(|id| layer.filters.iter().any(|f| f.id() == id))
                    })
                });
        if valid {
            Ok(())
        } else {
            Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Filter source or stable reference changed",
            ))
        }
    }
}
