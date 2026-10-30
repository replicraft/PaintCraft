//! Exact accepted activation scope follows its action through the existing FIFO.
use super::*;
#[derive(Debug)]
pub(in crate::controller) struct QueuedEdit {
    pub(in crate::controller) action: Action,
    pub(in crate::controller) scope: Option<Scope>,
    pub(in crate::controller) text: Option<text_submission::Attempt>,
}
#[derive(Debug, Default)]
pub(in crate::controller) struct Activations(paintcraft_ui::outbox::Outbox<QueuedEdit>);
impl Activations {
    pub(in crate::controller) fn push(
        &mut self,
        token: Token,
        action: Action,
        scope: Option<Scope>,
    ) -> bool {
        self.0.push(
            token,
            QueuedEdit {
                action,
                scope,
                text: None,
            },
        )
    }
    pub(in crate::controller) fn push_queued(&mut self, token: Token, queued: QueuedEdit) -> bool {
        self.0.push(token, queued)
    }
    pub(in crate::controller) fn revoke_filter(&mut self, edit: &paintcraft_tools::FilterEdit) {
        self.0
            .retain(|queued| queued.action != Action::Filter(edit.clone()));
    }
    pub(in crate::controller) fn revoke_text(&mut self, token: u64) {
        self.0
            .retain(|queued| queued.text.is_none_or(|attempt| attempt.token != token));
    }
    pub(in crate::controller) fn remember(&mut self, token: Token) -> bool {
        self.0.remember(token)
    }
    pub(in crate::controller) fn has_seen(&self, token: Token) -> bool {
        self.0.has_seen(token)
    }
    pub(in crate::controller) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub(in crate::controller) fn pending(&self) -> impl ExactSizeIterator<Item = &Action> {
        self.0.pending().map(|queued| &queued.action)
    }
    #[cfg(target_os = "macos")]
    pub(in crate::controller) fn has(&self, activation: Activation) -> bool {
        self.0.pending().any(|queued| {
            queued
                .scope
                .as_ref()
                .is_some_and(|s| s.activation == activation)
        })
    }
    #[cfg(target_os = "macos")]
    pub(in crate::controller) fn retire(&mut self, activation: Activation) {
        self.0.retain(|queued| {
            queued
                .scope
                .as_ref()
                .is_none_or(|s| s.activation != activation)
        });
    }
    pub(in crate::controller) fn drain(&mut self) -> Vec<QueuedEdit> {
        self.0.drain()
    }
}
impl Editor {
    pub(in crate::controller) fn drain_activations(&mut self) {
        self.actions.extend(
            self.outbox
                .drain()
                .into_iter()
                .map(background::Queued::Edit),
        );
    }
    pub(in crate::controller) fn execute_activation(&mut self, queued: QueuedEdit) {
        if let Err(error) =
            check(queued.scope.as_ref()).and_then(|_| self.start_text_attempt(queued.text))
        {
            self.fail_text_attempt(queued.text);
            if let Action::Filter(edit) = &queued.action {
                self.fail_queued_filter(edit);
            }
            self.status = error.to_string();
            return;
        }
        let prior = self.native_activation.take();
        self.native_activation = queued.scope;
        self.execute(queued.action);
        self.text_submission.executing = None;
        self.native_activation = prior;
    }
}
