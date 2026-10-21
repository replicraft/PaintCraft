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
        if prepared.session.owner != self.owner
            || prepared.base != self.ticket()
            || !Arc::ptr_eq(
                &prepared.session.preparation_policy,
                &self.preparation_policy,
            )
        {
            return Err(Error::new(
                ErrorCode::ConflictStaleBase,
                "Prepared edit no longer matches the session or its resource policy",
            ));
        }
        if prepared.receipt.committed {
            self.doc = prepared.session.doc;
            self.selection = prepared.session.selection;
            self.epoch = prepared.session.epoch;
            self.history = prepared.session.history;
        }
        Ok(prepared.receipt)
    }
}

impl Preparation {
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
