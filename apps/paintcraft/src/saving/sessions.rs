//! Per-document adoption state; queue admission and file ordering remain shared.
use super::*;

#[derive(Debug)]
pub(crate) struct SessionState {
    session: Uuid,
    generation: u64,
    adopted_sequence: u64,
    inspection: resumed::State,
    active_restore: Option<restoring::Control>,
    restore_intent: Option<restoring::Intent>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            session: Uuid::new_v4(),
            generation: 0,
            adopted_sequence: 0,
            inspection: Default::default(),
            active_restore: None,
            restore_intent: None,
        }
    }
}

impl SessionState {
    pub(crate) fn recovery_busy(&self) -> bool {
        self.active_restore.is_some() || self.inspection.busy()
    }

    pub(crate) fn owner(&self) -> Uuid {
        self.session
    }
}

impl NativeSaving {
    /// Resource adoption must not acquire a source already targeted by an
    /// admitted project write, regardless of which tab owns that write.
    pub(crate) fn has_pending_write_to(&self, path: &Path) -> bool {
        self.pending.iter().any(|pending| {
            matches!(&pending.kind, PendingKind::Save(request)
                if crate::tabs::same_destination(&request.path, path)
                    || crate::destination::same_file(&request.path, path))
        })
    }
    pub(crate) fn other_pending_destination(&self, path: &Path) -> bool {
        self.pending.iter().any(|p|matches!(&p.kind,PendingKind::Save(r) if r.session!=self.session && crate::tabs::same_destination(&r.path,path)))
    }
    pub(crate) fn pending_for_tab(&self, app: &App, id: Uuid) -> bool {
        self.pending
            .iter()
            .any(|p| self.completion_tab(app, &p.kind) == Some(id))
    }
    pub(crate) fn session_owner(&self) -> Uuid {
        self.session
    }
    pub(crate) fn swap_session(&mut self, other: &mut SessionState) {
        std::mem::swap(&mut self.session, &mut other.session);
        std::mem::swap(&mut self.generation, &mut other.generation);
        std::mem::swap(&mut self.adopted_sequence, &mut other.adopted_sequence);
        std::mem::swap(&mut self.inspection, &mut other.inspection);
        std::mem::swap(&mut self.active_restore, &mut other.active_restore);
        std::mem::swap(&mut self.restore_intent, &mut other.restore_intent);
    }

    fn completion_tab(&self, app: &App, kind: &PendingKind) -> Option<Uuid> {
        match kind {
            PendingKind::Save(r) => app.tab_for_session(r.session),
            PendingKind::Checkpoint { session, .. } | PendingKind::Initialize { session, .. } => {
                app.tab_for_session(*session)
            }
            PendingKind::Restore(r) => app.tab_for_session(r.owner()),
            PendingKind::Reconcile { sequence } => self
                .attempt_owner(*sequence)
                .and_then(|owner| app.tab_for_session(owner)),
            PendingKind::Resumed(r) => app.tab_for_session(r.owner()),
            PendingKind::OfferDiscard { .. } => None,
            PendingKind::Cleanup => None,
        }
    }
}

impl App {
    pub(super) fn route_native_completion(
        &mut self,
        ticket: Ticket,
        kind: PendingKind,
        result: Result<Completion>,
    ) {
        let original = self.active_tab_id();
        let target = self
            .saving
            .as_ref()
            .and_then(|s| s.completion_tab(self, &kind));
        if let Some(target) = target {
            self.swap_tab_state(target);
        }
        self.complete_native(ticket, kind, result);
        if target.is_some_and(|id| id != original)
            || self.dialog_owner.is_some()
            || self.confirm.is_some()
            || self.tab_closing.is_some()
        {
            self.swap_tab_state(original);
        }
    }
}
