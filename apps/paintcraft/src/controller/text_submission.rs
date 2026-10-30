//! One retained text intention at its exact FIFO boundary; no draft ownership transfer.
use super::*;
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_tools::TextStamp;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Attempt {
    stamp: TextStamp,
    pub(super) token: u64,
    owner: Uuid,
    document: Uuid,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Queued,
    Active,
    Failed,
}
#[derive(Clone, Copy, Debug)]
struct Placement {
    attempt: Attempt,
    phase: Phase,
}
#[derive(Debug, Default)]
pub(super) struct Tracker {
    placement: Option<Placement>,
    capture_failure: Option<(TextStamp, bool)>,
    pub(super) executing: Option<Attempt>,
    file_epoch: Uuid,
    next_text: Option<(Attempt, Uuid, (i64, i64))>,
}
impl Editor {
    pub fn type_text(&mut self, typed: &str) {
        let skipped = self.tools.type_text(typed);
        self.reconcile_text_submission();
        if skipped > 0 {
            self.status = format!("Skipped {skipped} character(s) the built-in font cannot draw");
        }
    }
    pub fn backspace(&mut self) {
        self.tools.backspace();
        self.reconcile_text_submission();
    }
    /// Automatic notifications coalesce, including an unchanged failed draft.
    pub fn commit_text(&mut self) {
        self.submit_text(false);
    }
    /// An explicit placement retries a failed stamp with a fresh attempt token.
    pub fn place_text(&mut self) {
        self.submit_text(true);
    }
    pub fn cancel_text(&mut self) {
        if self.retained_text_blocks_actions() {
            self.cancel_retained_text_queue();
        }
        self.tools.cancel_text();
        self.text_submission.placement = None;
        self.text_submission.capture_failure = None;
    }
    fn submit_text(&mut self, explicit: bool) {
        self.reconcile_text_submission();
        let Some(stamp) = self.tools.text_stamp() else {
            return;
        };
        if self
            .text_submission
            .capture_failure
            .is_some_and(|(failed, active)| failed == stamp && (!explicit || !active))
        {
            return;
        }
        if let Some(placement) = self.text_submission.placement
            && placement.attempt.stamp == stamp
            && (placement.phase != Phase::Failed || !explicit)
        {
            return;
        }
        let retry = self.text_placement_failed();
        let submission = match self.tools.capture_text_submission() {
            Ok(Some(submission)) => submission,
            Ok(None) => {
                self.tools.acknowledge_text(stamp);
                self.text_submission.placement = None;
                self.text_submission.capture_failure = None;
                self.actions
                    .retain(|queued| !matches!(queued, background::Queued::TextCaptureFailure(_)));
                return;
            }
            Err(error) => {
                self.queue_text_capture_failure(stamp, retry);
                self.status = format!("Cannot place text: {error}");
                return;
            }
        };
        let replacement = self.text_submission.capture_failure
            .filter(|(_, active)| !active)
            .and_then(|(stamp, _)| self.actions.iter().position(|queued|
                matches!(queued, background::Queued::TextCaptureFailure(current) if *current == stamp)));
        let (token, stamp, action) = submission.into_parts();
        let attempt = Attempt {
            stamp,
            token,
            owner: self.session.owner_id(),
            document: self.document().document_id,
        };
        let queued = native_publication::QueuedEdit {
            action,
            scope: self.native_activation.clone(),
            text: Some(attempt),
        };
        let accepted = if let Some(index) = replacement {
            if self.outbox.remember(token) {
                self.actions[index] = background::Queued::Edit(queued);
                true
            } else {
                false
            }
        } else if retry {
            if self.outbox.remember(token) {
                self.actions.push_front(background::Queued::Edit(queued));
                true
            } else {
                false
            }
        } else {
            self.outbox.push_queued(token, queued)
        };
        if accepted {
            self.text_submission.capture_failure = None;
            self.text_submission.placement = Some(Placement {
                attempt,
                phase: Phase::Queued,
            });
        }
        self.preview_queue_revision.advance();
        if !accepted {
            self.queue_text_capture_failure(stamp, retry);
        }
    }
    fn queue_text_capture_failure(&mut self, stamp: TextStamp, retry: bool) {
        let replacement = self.text_submission.capture_failure.filter(|(_, active)| !active)
            .and_then(|(prior, _)| self.actions.iter().position(|queued|
                matches!(queued, background::Queued::TextCaptureFailure(current) if *current == prior)));
        self.text_submission.placement = None;
        self.text_submission.capture_failure = Some((stamp, false));
        self.text_submission.next_text = None;
        if let Some(index) = replacement {
            self.actions[index] = background::Queued::TextCaptureFailure(stamp);
        } else if retry {
            self.actions
                .push_front(background::Queued::TextCaptureFailure(stamp));
        } else {
            self.drain_activations();
            self.actions
                .push_back(background::Queued::TextCaptureFailure(stamp));
        }
    }
    pub(super) fn reach_text_capture_failure(&mut self, stamp: TextStamp) {
        if self
            .text_submission
            .capture_failure
            .is_some_and(|(current, _)| current == stamp)
        {
            self.text_submission.capture_failure = Some((stamp, true));
            self.invalidate_text_files();
        }
    }
    pub(super) fn check_text_attempt(&self, attempt: Option<Attempt>) -> Result<()> {
        if let Some(attempt) = attempt {
            let current = self
                .text_submission
                .placement
                .is_some_and(|p| p.attempt == attempt && p.phase != Phase::Failed);
            if !current
                || !self.tools.text_stamp_current(attempt.stamp)
                || self.session.owner_id() != attempt.owner
                || self.document().document_id != attempt.document
            {
                return Err(Error::new(
                    ErrorCode::OperationCancelled,
                    "Text intention changed before placement",
                ));
            }
        }
        Ok(())
    }
    pub(super) fn start_text_attempt(&mut self, attempt: Option<Attempt>) -> Result<()> {
        self.check_text_attempt(attempt)?;
        if attempt.is_some() {
            self.text_submission.placement.as_mut().unwrap().phase = Phase::Active;
        }
        self.text_submission.executing = attempt;
        Ok(())
    }
    pub(super) fn finish_text_attempt(&mut self, attempt: Option<Attempt>, success: bool) {
        let Some(attempt) = attempt else {
            return;
        };
        if success && self.check_text_attempt(Some(attempt)).is_ok() {
            self.tools.acknowledge_text(attempt.stamp);
            self.text_submission.placement = None;
            if let Some((source, layer, at)) = self.text_submission.next_text.take()
                && source == attempt
                && self.tool == Tool::Text
                && self.target == Some(layer)
                && self.document().layers.contains_key(&layer)
            {
                self.tools.begin_text(layer, at);
            }
        } else {
            self.fail_text_attempt(Some(attempt));
        }
    }
    pub(super) fn fail_text_attempt(&mut self, attempt: Option<Attempt>) {
        if let Some(attempt) = attempt
            && self
                .text_submission
                .placement
                .is_some_and(|p| p.attempt == attempt && p.phase != Phase::Failed)
        {
            self.text_submission.placement.as_mut().unwrap().phase = Phase::Failed;
            self.text_submission.next_text = None;
            self.invalidate_text_files();
        }
    }
    pub(crate) fn cancel_next_text_pointer(&mut self) {
        self.text_submission.next_text = None;
    }
    pub(super) fn defer_next_text_pointer(
        &mut self,
        at: (i64, i64),
        button: paintcraft_input::PointerButton,
    ) {
        if self.tool == Tool::Text
            && button == paintcraft_input::PointerButton::Primary
            && let Some(placement) = self.text_submission.placement
            && placement.phase == Phase::Queued
            && let Some(layer) = self.target
        {
            self.text_submission.next_text = Some((placement.attempt, layer, at));
        }
    }
    pub(super) fn invalidate_text_files(&mut self) {
        self.text_submission.file_epoch = Uuid::new_v4();
        self.publication_preview = None;
        self.preview_queue_revision.advance();
        self.actions
            .retain(|action| !matches!(action, background::Queued::FileBarrier(_)));
        self.file_barrier = None;
    }
    fn cancel_retained_text_queue(&mut self) {
        self.drain_activations();
        let boundary = self.actions.iter().position(|queued| match queued {
            background::Queued::Edit(edit) => edit.text.is_some_and(|attempt| {
                self.text_submission
                    .placement
                    .is_some_and(|p| p.phase == Phase::Queued && p.attempt == attempt)
            }),
            background::Queued::TextCaptureFailure(stamp) => self
                .text_submission
                .capture_failure
                .is_some_and(|(current, active)| !active && current == *stamp),
            _ => false,
        });
        if let Some(boundary) = boundary {
            self.actions.truncate(boundary);
        } else {
            self.actions.clear();
            self.invalidate_text_files();
        }
        if let Some(pending) = &self.pending_edit
            && pending.text.is_some()
        {
            pending.cancel(); // Keep the handle and its leases until actual retirement.
            self.publication_preview = None;
        }
        if self
            .waiting_dispatch
            .as_ref()
            .is_some_and(|waiting| waiting.text.is_some())
        {
            self.waiting_dispatch = None;
            self.publication_preview = None;
        }
        if self
            .waiting_edit
            .as_ref()
            .is_some_and(|waiting| waiting.text.is_some())
        {
            self.waiting_edit = None;
            self.publication_preview = None;
        }
        if self.actions.is_empty() && !self.has_inflight_edit() {
            self.publication_preview = None;
        }
        self.text_submission.next_text = None;
        self.preview_queue_revision.advance();
    }
    #[cfg(target_os = "macos")]
    pub(super) fn retire_queued_text(&mut self, attempt: Attempt) -> TextStamp {
        self.cancel_next_text_pointer();
        if self
            .text_submission
            .placement
            .is_some_and(|p| p.attempt == attempt)
        {
            self.text_submission.placement = None;
            self.text_submission.capture_failure = Some((attempt.stamp, false));
        }
        attempt.stamp
    }
    pub(crate) fn text_file_epoch(&self) -> Uuid {
        self.text_submission.file_epoch
    }
    pub(super) fn text_placement_failed(&self) -> bool {
        self.text_submission
            .placement
            .is_some_and(|p| p.phase == Phase::Failed)
            || self
                .text_submission
                .capture_failure
                .is_some_and(|(_, active)| active)
    }
    pub(crate) fn failed_text_retained(&self) -> bool {
        self.text_placement_failed()
    }
    pub(super) fn retained_text_blocks_actions(&self) -> bool {
        self.text_submission.placement.is_some() || self.text_submission.capture_failure.is_some()
    }
    pub(super) fn reconcile_text_submission(&mut self) {
        let Some(placement) = self.text_submission.placement else {
            return;
        };
        if placement.phase == Phase::Failed
            || self.check_text_attempt(Some(placement.attempt)).is_ok()
        {
            return;
        }
        if placement.phase == Phase::Queued {
            self.drain_activations();
            let mut replaced = false;
            for queued in &mut self.actions {
                if matches!(queued, background::Queued::Edit(edit) if edit.text == Some(placement.attempt))
                {
                    *queued = background::Queued::TextCaptureFailure(placement.attempt.stamp);
                    replaced = true;
                }
            }
            if replaced {
                self.text_submission.placement = None;
                self.text_submission.capture_failure = Some((placement.attempt.stamp, false));
                self.text_submission.next_text = None;
                self.preview_queue_revision.advance();
                return;
            }
        }
        if let Some(pending) = &self.pending_edit
            && pending.text == Some(placement.attempt)
        {
            pending.cancel();
        }
        if self
            .waiting_dispatch
            .as_ref()
            .is_some_and(|p| p.text == Some(placement.attempt))
        {
            self.waiting_dispatch = None;
        }
        if self
            .waiting_edit
            .as_ref()
            .is_some_and(|p| p.text == Some(placement.attempt))
        {
            self.waiting_edit = None;
        }
        self.outbox.revoke_text(placement.attempt.token);
        self.actions.retain(|action| !matches!(action, background::Queued::Edit(queued) if queued.text == Some(placement.attempt)));
        self.fail_text_attempt(Some(placement.attempt));
        self.publication_preview = None;
    }
    /// Saves must never capture an older canvas while authored text is retained.
    pub(crate) fn text_save_ready(&self) -> bool {
        if self.filter_edit_open {
            return false;
        }
        if self.file_barrier.is_some()
            && !self.text_placement_failed()
            && self
                .text_submission
                .placement
                .is_none_or(|placement| placement.phase == Phase::Queued)
        {
            return true;
        } // Its accepted prefix precedes the retained later intention.
        self.tools.text_content().is_none_or(str::is_empty)
            && self.text_submission.placement.is_none()
            && self.text_submission.capture_failure.is_none()
    }
    pub(crate) fn has_processing_edits(&self) -> bool {
        self.has_inflight_edit() || (!self.text_placement_failed() && self.has_pending_edits())
    }
    #[cfg(test)]
    pub(crate) fn test_persistent_validator(
        &mut self,
        validator: Option<paintcraft_editor::session::PersistentValidator>,
    ) {
        self.session.set_persistent_validator(validator);
    }
    /// A failed FIFO boundary excludes its blocked suffix from private exports.
    /// Its own older worker must have completed, not merely received cancellation.
    pub(crate) fn can_capture_export(&self) -> bool {
        !self.filter_edit_open
            && !self.has_image_paste()
            && if self.text_placement_failed() {
                !self.has_inflight_edit() && self.maintenance.is_none()
            } else {
                self.can_capture_file()
            }
    }
}

#[cfg(test)]
#[path = "text_submission/tests.rs"]
mod tests;
