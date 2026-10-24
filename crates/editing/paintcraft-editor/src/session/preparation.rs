//! Detached transaction preparation; publication remains on the session owner.
use super::*;

/// A private timeline sharing immutable payloads with its source session.
/// Send this value to a bounded worker, then return its prepared result.
#[derive(Debug)]
pub struct Preparation {
    pub(super) base: Ticket,
    pub(super) session: Session,
}

/// A fully prepared edit, including admitted history and flushed spill extents.
/// Dropping it discards its unpublished state and releases its payload leases.
#[derive(Debug)]
pub struct PreparedEdit {
    base: Ticket,
    session: Session,
    receipt: Receipt,
}

/// A restored private history target, ready for guarded owner publication.
#[derive(Debug)]
pub struct PreparedNavigation {
    base: Ticket,
    session: Session,
    navigation: Option<Navigation>,
}

impl Session {
    /// Capture a detached timeline. This only copies metadata (fallibly, under
    /// the history staging allowance); pixel execution and pressure work happen
    /// in `Preparation::apply`, on the caller's chosen worker.
    pub fn preparation(&self) -> Result<Preparation> {
        Ok(Preparation {
            base: self.ticket(),
            session: Self {
                owner: self.owner,
                doc: self.doc.clone(),
                selection: self.selection,
                epoch: self.epoch,
                history: self.history.try_fork()?,
                saved_state: self.saved_state,
                staged_pixels: self.staged_pixels,
                preparation_policy: self.preparation_policy.clone(),
            },
        })
    }

    /// Publish once against the captured session, epoch and resource policy.
    /// A save completed during preparation keeps its independently acknowledged
    /// saved state. Foreign, stale or cancelled results publish nothing.
    pub fn publish_prepared(
        &mut self,
        prepared: PreparedEdit,
        check: impl Fn() -> Result<()>,
    ) -> Result<Receipt> {
        check()?;
        self.validate_preparation(prepared.base, &prepared.session)?;
        if prepared.receipt.committed {
            self.doc = prepared.session.doc;
            self.selection = prepared.session.selection;
            self.epoch = prepared.session.epoch;
            self.history = prepared.session.history;
        }
        Ok(prepared.receipt)
    }

    /// Publish a restored history target under the same guard as an edit.
    pub fn publish_navigation(
        &mut self,
        prepared: PreparedNavigation,
        check: impl Fn() -> Result<()>,
    ) -> Result<Option<Navigation>> {
        check()?;
        self.validate_preparation(prepared.base, &prepared.session)?;
        if prepared.navigation.is_some() {
            self.doc = prepared.session.doc;
            self.selection = prepared.session.selection;
            self.epoch = prepared.session.epoch;
            self.history = prepared.session.history;
        }
        Ok(prepared.navigation)
    }

    pub(super) fn validate_preparation(&self, base: Ticket, source: &Session) -> Result<()> {
        if source.owner != self.owner
            || base != self.ticket()
            || !Arc::ptr_eq(&source.preparation_policy, &self.preparation_policy)
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Prepared transaction no longer matches the session or its resource policy",
            ));
        }
        Ok(())
    }
}

impl Preparation {
    pub fn history_ticket(&self) -> HistoryTicket {
        HistoryTicket {
            owner: self.session.owner,
            base: self.base,
        }
    }
    /// Finish a tool preparation that produced no operations (for example,
    /// fully clipped text). Publication still checks the captured owner/base/policy.
    pub fn no_change(self, check: impl Fn() -> Result<()>) -> Result<PreparedEdit> {
        check()?;
        let receipt = Receipt {
            committed: false,
            state_id: self.base.state_id,
            publication_epoch: self.base.publication_epoch,
            history_id: None,
            aliases: BTreeMap::new(),
        };
        Ok(PreparedEdit {
            base: self.base,
            session: self.session,
            receipt,
        })
    }

    /// Execute privately and check cancellation around command execution and
    /// throughout history classification and spill batches. Smooth coverage, flood classification and planned
    /// paint replay yield cooperatively; other command kernels and filesystem calls
    /// are not preemptible.
    pub fn apply(self, batch: &Batch, check: impl Fn() -> Result<()>) -> Result<PreparedEdit> {
        let admission = Admission::new(self.session.staged_pixels);
        self.apply_with_admission(batch, admission, check)
    }

    /// Execute with caller-admitted decoded capacity while preserving the session's
    /// staged limit, cancellation, history and owner publication guards.
    /// History decode/codec resources retain their separate admission policies.
    pub fn apply_with_admission(
        mut self,
        batch: &Batch,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedEdit> {
        let receipt = self
            .session
            .apply_checked_with_admission(batch, admission, &check)?;
        check()?;
        Ok(PreparedEdit {
            base: self.base,
            session: self.session,
            receipt,
        })
    }
}

impl Preparation {
    pub fn undo(self, check: impl Fn() -> Result<()>) -> Result<PreparedNavigation> {
        self.undo_with_admission(Admission::new(u64::MAX), check)
    }
    pub fn undo_with_admission(
        mut self,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedNavigation> {
        let navigation = self.session.undo_with_admission(admission, &check)?;
        self.navigation(navigation, check)
    }
    pub fn redo(self, check: impl Fn() -> Result<()>) -> Result<PreparedNavigation> {
        self.redo_with_admission(Admission::new(u64::MAX), check)
    }
    pub fn redo_with_admission(
        mut self,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedNavigation> {
        let navigation = self.session.redo_with_admission(admission, &check)?;
        self.navigation(navigation, check)
    }
    pub fn navigate_history(
        self,
        ticket: HistoryTicket,
        target: Option<Uuid>,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedNavigation> {
        self.navigate_history_with_admission(ticket, target, Admission::new(u64::MAX), check)
    }
    pub fn navigate_history_with_admission(
        mut self,
        ticket: HistoryTicket,
        target: Option<Uuid>,
        admission: Admission,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedNavigation> {
        let navigation = self
            .session
            .navigate_history_with_admission(ticket, target, admission, &check)?;
        self.navigation(navigation, check)
    }

    fn navigation(
        self,
        navigation: Option<Navigation>,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedNavigation> {
        check()?;
        Ok(PreparedNavigation {
            base: self.base,
            session: self.session,
            navigation,
        })
    }
}

impl PreparedEdit {
    /// Freeze a complete prepared revision for exporting a copy. This does not
    /// publish it or acknowledge any save on the owning session.
    pub fn document_snapshot(&self) -> Arc<Document> {
        self.session.snapshot()
    }
}
