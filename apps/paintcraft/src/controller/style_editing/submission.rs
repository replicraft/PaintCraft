//! One native style Apply lifetime, retained until an actual publication receipt.
use super::super::*;
use paintcraft_core::types::{Error, ErrorCode};
use paintcraft_tools::StyleEdit;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Feedback {
    Queued,
    Active,
    Failed,
    Accepted,
    Cancelled,
}
#[derive(Debug)]
struct Attempt {
    id: Uuid,
    token: Token,
    edit: StyleEdit,
    feedback: Feedback,
}
#[derive(Debug, Default)]
pub(in crate::controller) struct Tracker {
    current: Option<Attempt>,
    pub(in crate::controller) executing: Option<Uuid>,
}
impl Editor {
    pub(crate) fn queue_style_apply(&mut self, token: Token, edit: StyleEdit) -> Result<Uuid> {
        self.validate_style_edit(&edit)?;
        if let Some(attempt) = &self.style_editing.submission.current
            && matches!(attempt.feedback, Feedback::Queued | Feedback::Active)
        {
            if attempt.token == token && attempt.edit == edit {
                return Ok(attempt.id);
            }
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "A style Apply is already active",
            ));
        }
        let id = Uuid::new_v4();
        self.style_editing.submission.current = Some(Attempt {
            id,
            token,
            edit: edit.clone(),
            feedback: Feedback::Queued,
        });
        if !self.request(token, Action::Style(edit)) {
            self.finish_style_attempt(Some(id), false);
        }
        Ok(id)
    }
    pub(crate) fn style_feedback(&self, id: Uuid) -> Option<Feedback> {
        self.style_editing
            .submission
            .current
            .as_ref()
            .filter(|a| a.id == id)
            .map(|a| a.feedback)
    }
    pub(in crate::controller) fn style_request_allowed(
        &self,
        token: Token,
        edit: &StyleEdit,
    ) -> bool {
        self.style_editing
            .submission
            .current
            .as_ref()
            .is_some_and(|a| a.token == token && a.edit == *edit && a.feedback == Feedback::Queued)
    }
    pub(super) fn start_style_attempt(&mut self, edit: &StyleEdit) -> Option<Uuid> {
        let attempt = self.style_editing.submission.current.as_mut()?;
        if attempt.edit != *edit || attempt.feedback != Feedback::Queued {
            return None;
        }
        attempt.feedback = Feedback::Active;
        self.style_editing.submission.executing = Some(attempt.id);
        Some(attempt.id)
    }
    pub(in crate::controller) fn check_style_attempt(&self, id: Option<Uuid>) -> Result<()> {
        let Some(id) = id else {
            return Ok(());
        };
        let current = self.style_editing.submission.current.as_ref().filter(|a| {
            a.id == id
                && a.feedback == Feedback::Active
                && self.validate_style_edit(&a.edit).is_ok()
        });
        if current.is_some() {
            Ok(())
        } else {
            Err(Error::new(
                ErrorCode::OperationCancelled,
                "Style Apply lifetime changed",
            ))
        }
    }
    pub(in crate::controller) fn finish_style_attempt(
        &mut self,
        id: Option<Uuid>,
        published: bool,
    ) {
        if let Some(attempt) = self.style_editing.submission.current.as_mut()
            && Some(attempt.id) == id
            && attempt.feedback != Feedback::Cancelled
        {
            attempt.feedback = if published {
                Feedback::Accepted
            } else {
                Feedback::Failed
            };
        }
    }
    pub(in crate::controller) fn fail_queued_style(&mut self, edit: &StyleEdit) {
        let id = self
            .style_editing
            .submission
            .current
            .as_ref()
            .filter(|a| a.edit == *edit)
            .map(|a| a.id);
        self.finish_style_attempt(id, false);
    }
    pub(crate) fn cancel_style_attempt(&mut self, id: Uuid) {
        let Some(attempt) = self
            .style_editing
            .submission
            .current
            .as_mut()
            .filter(|a| a.id == id)
        else {
            return;
        };
        attempt.feedback = Feedback::Cancelled;
        let edit = attempt.edit.clone();
        self.outbox.revoke_style(&edit);
        self.actions.retain(
            |a| !matches!(a, background::Queued::Edit(q) if q.action==Action::Style(edit.clone())),
        );
        if let Some(pending) = &self.pending_edit
            && pending.style == Some(id)
        {
            pending.cancel();
        }
        if self
            .waiting_dispatch
            .as_ref()
            .is_some_and(|w| w.style == Some(id))
        {
            self.waiting_dispatch = None;
        }
        if self
            .waiting_edit
            .as_ref()
            .is_some_and(|w| w.style == Some(id))
        {
            self.waiting_edit = None;
        }
        self.publication_preview = None;
        self.preview_queue_revision.advance();
    }
}
