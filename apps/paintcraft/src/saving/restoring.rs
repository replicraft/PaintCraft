//! Source claims live until adoption and a confirmed destination checkpoint/save.
//! Preparing and disposing decoded documents happens on the same admitted I/O worker.
use super::*;
use paintcraft_formats::recovery::{ClaimedRecovery, RecoveryLease};
use std::sync::atomic::{AtomicBool, Ordering};
const READ_CHARGE: u64 = 1 << 30;
const CONTROL_CHARGE: u64 = 64 * 1024;

#[derive(Debug)]
pub(crate) struct Handoff {
    lease: RecoveryLease,
    protected: bool,
    checkpoint_requested: bool,
}
impl Handoff {
    pub(crate) fn new(lease: RecoveryLease) -> Self {
        Self {
            lease,
            protected: false,
            checkpoint_requested: false,
        }
    }
    pub(super) fn protect(&mut self) {
        self.protected = true;
    }
    pub(crate) fn discard(self) -> Result<()> {
        self.lease.discard()
    }
}
#[derive(Debug)]
pub(super) struct Control {
    id: Uuid,
    cancelled: Arc<AtomicBool>,
}
#[derive(Debug)]
pub(crate) struct Drafts {
    name: Option<(Uuid, String)>,
    opacity: Option<(Uuid, u8)>,
}
impl Drafts {
    pub(crate) fn capture(app: &App) -> Self {
        Self {
            name: app.name_edit.clone(),
            opacity: app.opacity_edit,
        }
    }
    fn matches(&self, app: &App) -> bool {
        self.name == app.name_edit && self.opacity == app.opacity_edit
    }
}
#[derive(Debug)]
pub(super) struct Request {
    id: Uuid,
    session: Uuid,
    scope: Uuid,
    generation: u64,
    key: (Uuid, u64),
    offer: Found,
    drafts: Drafts,
}
#[derive(Debug)]
pub(super) struct Intent {
    request: Request,
    cancelled: Arc<AtomicBool>,
}
impl NativeSaving {
    pub(crate) fn restoring(&self) -> bool {
        self.active_restore.is_some()
    }
    pub(crate) fn cancel_restore(&mut self) {
        if let Some(control) = self.active_restore.take() {
            control.cancelled.store(true, Ordering::Release);
        }
        self.restore_intent = None;
    }
    pub(crate) fn request_restore(
        &mut self,
        editor: &Editor,
        scope: Uuid,
        offer: Found,
        drafts: Drafts,
    ) -> Result<()> {
        if self.restoring() {
            return Ok(());
        }
        let id = Uuid::new_v4();
        let cancelled = Arc::new(AtomicBool::new(false));
        self.active_restore = Some(Control {
            id,
            cancelled: cancelled.clone(),
        });
        self.restore_intent = Some(Intent {
            request: Request {
                id,
                session: self.session,
                scope,
                generation: self.generation,
                key: editor.render_key(),
                offer,
                drafts,
            },
            cancelled,
        });
        self.try_restore(editor, scope)
    }
    fn try_restore(&mut self, editor: &Editor, scope: Uuid) -> Result<()> {
        let Some(intent) = &self.restore_intent else {
            return Ok(());
        };
        if intent.request.session != self.session
            || intent.request.scope != scope
            || intent.request.generation != self.generation
            || intent.request.key != editor.render_key()
            || editor.gesture_in_progress()
            || editor.is_editing_text()
        {
            self.cancel_restore();
            return Err(Error::new(
                ErrorCode::OperationCancelled,
                "current document changed before recovery could start",
            ));
        }
        if self.io.admit(1, READ_CHARGE).is_err() {
            return Ok(());
        }
        let intent = self.restore_intent.take().expect("pending intent");
        let offer = intent.request.offer.clone();
        let context = self.context.clone();
        let ticket = self.io.submit(
            READ_CHARGE,
            move || {
                if intent.cancelled.load(Ordering::Acquire) {
                    return Err(Error::new(
                        ErrorCode::OperationCancelled,
                        "recovery preparation cancelled",
                    ));
                }
                Recovery::claim(&offer).map(|claim| Completion::Claimed(Box::new(claim)))
            },
            move || context.request_repaint(),
        );
        match ticket {
            Ok(ticket) => {
                self.pending.push(Pending {
                    ticket,
                    kind: PendingKind::Restore(Box::new(intent.request)),
                });
                Ok(())
            }
            Err(error) => {
                self.cancel_restore();
                Err(error)
            }
        }
    }
    pub(crate) fn reserve_controls(&self, count: usize) -> Result<()> {
        let bytes = (count as u64).checked_mul(CONTROL_CHARGE).ok_or_else(|| {
            Error::new(
                ErrorCode::ResourceLimit,
                "recovery control charge overflows",
            )
        })?;
        self.io.admit(count, bytes)
    }
    pub(crate) fn discard_source(&mut self, source: Handoff) -> Result<()> {
        let context = self.context.clone();
        let ticket = self.io.submit(
            CONTROL_CHARGE,
            move || {
                source.discard()?;
                Ok(Completion::Cleanup)
            },
            move || context.request_repaint(),
        )?;
        self.pending.push(Pending {
            ticket,
            kind: PendingKind::Cleanup,
        });
        Ok(())
    }
    pub(crate) fn discard_offer(&mut self, scope: Uuid, offer: &Found) -> Result<()> {
        if self.pending.iter().any(|pending| matches!(pending.kind, PendingKind::OfferDiscard { scope: existing_scope, session } if existing_scope == scope && session == offer.session)) { return Ok(()); }

        let directory = offer
            .checkpoint
            .parent()
            .ok_or_else(|| {
                Error::new(ErrorCode::TargetInvalid, "recovery source has no directory")
            })?
            .to_owned();
        let session = offer.session;
        let context = self.context.clone();
        let ticket = self.io.submit(
            CONTROL_CHARGE,
            move || {
                Recovery::discard(&directory, session)?;
                Ok(Completion::Cleanup)
            },
            move || context.request_repaint(),
        )?;
        self.pending.push(Pending {
            ticket,
            kind: PendingKind::OfferDiscard { scope, session },
        });
        Ok(())
    }
    pub(crate) fn offer_busy(&self, offer: &Found) -> bool {
        self.restoring() || self.pending.iter().any(|p| matches!(p.kind, PendingKind::OfferDiscard { session, .. } if session == offer.session))
    }
    fn dispose_claim(&mut self, claim: Box<ClaimedRecovery>) -> Result<()> {
        let context = self.context.clone();
        let ticket = self.io.submit(
            READ_CHARGE,
            move || {
                drop(claim);
                Ok(Completion::Cleanup)
            },
            move || context.request_repaint(),
        )?;
        self.pending.push(Pending {
            ticket,
            kind: PendingKind::Cleanup,
        });
        Ok(())
    }
}
impl App {
    pub(crate) fn protect_recovery_source(&mut self) {
        if let Some(source) = &mut self.recovery_source {
            source.protect();
        }
    }
    pub(super) fn pump_restore(&mut self) {
        if let Some(saving) = &mut self.saving
            && let Err(error) = saving.try_restore(&self.editor, self.recovery_scope)
        {
            self.editor
                .set_status(format!("Recovery not applied: {error}"));
        }
    }
    pub(crate) fn pump_recovery_handoff(&mut self) {
        let Some(source) = &self.recovery_source else {
            return;
        };
        if source.protected {
            if let Some(saving) = &mut self.saving {
                if saving.reserve_controls(1).is_err() {
                    return;
                }
                let source = self.recovery_source.take().expect("protected source");
                match saving.discard_source(source) {
                    Ok(())
                        if self.editor.status().starts_with("Recovered;")
                            || self.editor.status().starts_with("Autosave failed:") =>
                    {
                        self.editor.set_status("Recovered copy is now protected.")
                    }
                    Ok(()) => {}
                    Err(error) => self
                        .editor
                        .set_status(format!("Recovered; original cleanup not queued: {error}")),
                }
            } else {
                let source = self.recovery_source.take().expect("protected source");
                if let Err(error) = source.discard() {
                    self.editor.set_status(format!(
                        "Recovered; previous recovery cleanup failed: {error}"
                    ));
                }
            }
        } else if !source.checkpoint_requested
            && let Some(saving) = &mut self.saving
        {
            match saving.checkpoint(&self.editor) {
                Ok(true) => {
                    self.recovery_source
                        .as_mut()
                        .expect("pending handoff")
                        .checkpoint_requested = true
                }
                Ok(false) => {}
                Err(error) if error.code == ErrorCode::ResourceLimit => {} // retry after an admission is consumed
                Err(error) => {
                    self.recovery_source
                        .as_mut()
                        .expect("pending handoff")
                        .checkpoint_requested = true;
                    self.editor.set_status(format!("Recovered; the original copy is retained until a checkpoint can be confirmed: {error}"));
                }
            }
        }
    }
    pub(super) fn apply_claimed_recovery(&mut self, request: Request, result: Result<Completion>) {
        let drafts_unchanged = request.drafts.matches(self);
        let saving = self.saving.as_mut().expect("native restore service");
        let active = saving
            .active_restore
            .as_ref()
            .is_some_and(|active| active.id == request.id);
        let current = active
            && drafts_unchanged
            && request.session == saving.session
            && request.scope == self.recovery_scope
            && request.generation == saving.generation
            && request.key == self.editor.render_key()
            && !self.editor.gesture_in_progress()
            && !self.editor.is_editing_text()
            && !self.close_after_saves
            && !self.allow_close;
        if active {
            saving.cancel_restore();
        }
        let claim = match result {
            Ok(Completion::Claimed(claim)) => claim,
            Err(error) => {
                if current && error.code != ErrorCode::OperationCancelled {
                    self.editor
                        .set_status(format!("Recovery was not applied: {error}"));
                }
                return;
            }
            _ => unreachable!("typed restore completion"),
        };
        if !current {
            if let Err(error) = saving.dispose_claim(claim) {
                saving.failed = true;
                self.editor
                    .set_status(format!("Recovery disposal failed: {error}"));
            } else if active {
                self.editor
                    .set_status("Recovery was not applied because the current document changed.");
            }
            return;
        }
        if let Err(error) = self.prepare_editor_replacement() {
            self.editor
                .set_status(format!("Recovery not applied: {error}"));
            if let Err(error) = self
                .saving
                .as_mut()
                .expect("native restore")
                .dispose_claim(claim)
            {
                self.editor
                    .set_status(format!("Recovery disposal failed: {error}"));
            }
            return;
        }
        let ClaimedRecovery { document, lease } = *claim;
        self.publish_editor(Editor::recovered_document(document));
        self.recovery_source = Some(Handoff::new(lease));
        self.bound = None;
        if let Some(original) = request.offer.original {
            self.path_text = original;
        }
        self.recoverable
            .retain(|offer| offer.session != request.offer.session);
        self.editor.set_status(
            "Recovered; keeping the original copy until a new checkpoint is confirmed…",
        );
    }
}

#[cfg(test)]
mod tests;
