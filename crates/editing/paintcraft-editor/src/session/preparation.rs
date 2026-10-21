//! Detached transaction preparation; publication remains on the session owner.
use super::*;

/// A private timeline sharing immutable payloads with its source session.
/// Send this value to a bounded worker, then return its prepared result.
#[derive(Debug)]
pub struct Preparation {
    base: Ticket,
    session: Session,
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

    fn validate_preparation(&self, base: Ticket, source: &Session) -> Result<()> {
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
    /// throughout history classification and spill batches. Individual command
    /// kernels and filesystem calls are not preemptible.
    pub fn apply(mut self, batch: &Batch, check: impl Fn() -> Result<()>) -> Result<PreparedEdit> {
        let receipt = self.session.apply_checked(batch, &check)?;
        check()?;
        Ok(PreparedEdit {
            base: self.base,
            session: self.session,
            receipt,
        })
    }
}

impl Preparation {
    pub fn undo(mut self, check: impl Fn() -> Result<()>) -> Result<PreparedNavigation> {
        let navigation = self.session.undo_checked(&check)?;
        self.navigation(navigation, check)
    }

    pub fn redo(mut self, check: impl Fn() -> Result<()>) -> Result<PreparedNavigation> {
        let navigation = self.session.redo_checked(&check)?;
        self.navigation(navigation, check)
    }

    pub fn navigate_history(
        mut self,
        ticket: HistoryTicket,
        target: Option<Uuid>,
        check: impl Fn() -> Result<()>,
    ) -> Result<PreparedNavigation> {
        let navigation = self
            .session
            .navigate_history_checked(ticket, target, &check)?;
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
