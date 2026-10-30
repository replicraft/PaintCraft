//! One native filter Apply lifetime, retained until an actual publication receipt.
use super::*;
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_tools::FilterEdit;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Feedback {
    Queued,
    Active,
    Failed,
    Published,
    Cancelled,
}
#[derive(Debug)]
struct Attempt {
    id: Uuid,
    token: Token,
    edit: FilterEdit,
    feedback: Feedback,
}
#[derive(Debug, Default)]
pub(super) struct Tracker {
    current: Option<Attempt>,
    pub(super) executing: Option<Uuid>,
}
impl Editor {
    pub(crate) fn queue_filter_apply(&mut self, token: Token, edit: FilterEdit) -> Result<Uuid> {
        self.validate_filter_edit(&edit)?;
        if let Some(attempt) = &self.filter_submission.current
            && matches!(attempt.feedback, Feedback::Queued | Feedback::Active)
        {
            return Ok(attempt.id);
        }
        let id = Uuid::new_v4();
        self.filter_submission.current = Some(Attempt {
            id,
            token,
            edit: edit.clone(),
            feedback: Feedback::Queued,
        });
        if !self.request(token, Action::Filter(edit)) {
            self.finish_filter_attempt(Some(id), false);
        }
        Ok(id)
    }
    pub(crate) fn filter_feedback(&self, id: Uuid) -> Option<Feedback> {
        self.filter_submission
            .current
            .as_ref()
            .filter(|a| a.id == id)
            .map(|a| a.feedback)
    }
    pub(super) fn filter_request_allowed(&self, token: Token, edit: &FilterEdit) -> bool {
        self.filter_submission
            .current
            .as_ref()
            .is_some_and(|a| a.token == token && a.edit == *edit && a.feedback == Feedback::Queued)
    }
    pub(super) fn start_filter_attempt(&mut self, edit: &FilterEdit) -> Option<Uuid> {
        let attempt = self.filter_submission.current.as_mut()?;
        if attempt.edit != *edit || attempt.feedback != Feedback::Queued {
            return None;
        }
        attempt.feedback = Feedback::Active;
        self.filter_submission.executing = Some(attempt.id);
        Some(attempt.id)
    }
    pub(super) fn check_filter_attempt(&self, id: Option<Uuid>) -> Result<()> {
        let Some(id) = id else {
            return Ok(());
        };
        let current = self.filter_submission.current.as_ref().filter(|a| {
            a.id == id && a.feedback == Feedback::Active && self.filter_base_current(&a.edit)
        });
        if current.is_some() {
            Ok(())
        } else {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "Filter Apply lifetime changed",
            ))
        }
    }
    pub(super) fn finish_filter_attempt(&mut self, id: Option<Uuid>, published: bool) {
        if let Some(attempt) = self.filter_submission.current.as_mut()
            && Some(attempt.id) == id
            && attempt.feedback != Feedback::Cancelled
        {
            attempt.feedback = if published {
                Feedback::Published
            } else {
                Feedback::Failed
            };
        }
    }
    pub(crate) fn cancel_active_filter(&mut self) {
        if let Some(a) = &self.filter_submission.current
            && matches!(a.feedback, Feedback::Queued | Feedback::Active)
        {
            self.cancel_filter_attempt(a.id);
        }
    }
    pub(super) fn fail_queued_filter(&mut self, edit: &FilterEdit) {
        let id = self
            .filter_submission
            .current
            .as_ref()
            .filter(|a| a.edit == *edit)
            .map(|a| a.id);
        self.finish_filter_attempt(id, false);
    }
    pub(crate) fn cancel_filter_attempt(&mut self, id: Uuid) {
        let Some(attempt) = self
            .filter_submission
            .current
            .as_mut()
            .filter(|a| a.id == id)
        else {
            return;
        };
        attempt.feedback = Feedback::Cancelled;
        let edit = attempt.edit.clone();
        self.outbox.revoke_filter(&edit);
        self.actions.retain(
            |a| !matches!(a, background::Queued::Edit(q) if q.action==Action::Filter(edit.clone())),
        );
        if let Some(pending) = &self.pending_edit
            && pending.filter == Some(id)
        {
            pending.cancel();
        }
        if self
            .waiting_dispatch
            .as_ref()
            .is_some_and(|w| w.filter == Some(id))
        {
            self.waiting_dispatch = None;
        }
        if self
            .waiting_edit
            .as_ref()
            .is_some_and(|w| w.filter == Some(id))
        {
            self.waiting_edit = None;
        }
        self.publication_preview = None;
        self.preview_queue_revision.advance();
    }
}
