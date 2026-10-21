//! Keep an admitted edit at its FIFO position while decoded capacity is unavailable.
use super::*;
use background::Completion;
use paintcraft_core::types::{
    ErrorCode,
    resources::{AllocationClass, AllocationDenial},
};
use paintcraft_editor::session::{HistoryTicket, Preparation};
use paintcraft_raster::Admission;

#[derive(Debug)]
pub(super) struct Deferred {
    batch: Arc<Batch>,
    base: HistoryTicket,
    denial: AllocationDenial,
}
#[derive(Debug)]
pub(super) struct Waiting {
    pub(super) deferred: Deferred,
    pub(super) done: String,
    pub(super) target_alias: Option<&'static str>,
}
pub(super) fn prepare_batch(
    preparation: Preparation,
    batch: Arc<Batch>,
    admission: Admission,
    check: impl Fn() -> Result<()>,
) -> Result<Completion> {
    let base = preparation.history_ticket();
    match preparation.apply_with_admission(&batch, admission, &check) {
        Ok(prepared) => Ok(Completion::Edit(prepared)),
        Err(error) => {
            check()?;
            match error.allocation_denial().filter(|denial| {
                denial.class() == AllocationClass::DecodedTiles && denial.is_possible()
            }) {
                Some(denial) => Ok(Completion::Deferred(Box::new(Deferred {
                    batch,
                    base,
                    denial: denial.clone(),
                }))),
                None => Err(error),
            }
        }
    }
}
impl Editor {
    pub(super) fn has_inflight_edit(&self) -> bool {
        self.pending_edit.is_some()
            || self.waiting_edit.is_some()
            || self.waiting_dispatch.is_some()
    }
    pub(crate) fn allocation_wait(&self) -> Option<&AllocationDenial> {
        self.waiting_edit
            .as_ref()
            .filter(|waiting| waiting.deferred.base == self.session.history_ticket())
            .map(|waiting| &waiting.deferred.denial)
    }
    pub(super) fn retry_waiting_edit(&mut self) {
        let Some(waiting) = &self.waiting_edit else {
            return;
        };
        if waiting.deferred.base != self.session.history_ticket() {
            self.waiting_edit = None;
            self.publication_preview = None;
            self.preview_queue_revision.advance();
            self.preview_lineage = Uuid::new_v4();
            self.status = "Waiting edit cancelled: its document base changed".into();
            return;
        }
        let Some(service) = self.editing.clone().filter(|service| service.available()) else {
            return;
        };
        if !waiting.deferred.denial.can_retry() {
            return;
        }
        // Acquire before capture/dispatch. A race retains the command for another wake.
        let Ok(reservation) = waiting.deferred.denial.reserve_capacity() else {
            return;
        };
        let preparation = match self.session.preparation() {
            Ok(preparation) => preparation,
            Err(error) => {
                self.waiting_edit = None;
                self.publication_preview = None;
                self.preview_queue_revision.advance();
                self.preview_lineage = Uuid::new_v4();
                self.status = format!("Edit not prepared: {error}");
                return;
            }
        };
        let batch = waiting.deferred.batch.clone();
        let admission = Admission::with_reserved_residency(
            paintcraft_editor::execute::STAGED_PIXELS_MAX,
            reservation,
        );
        let result = service.submit(
            move |cancellation| {
                prepare_batch(preparation, batch, admission, || cancellation.check())
            },
            waiting.done.clone(),
            "",
            waiting.target_alias,
        );
        match result {
            Ok(pending) => {
                self.waiting_edit = None;
                self.pending_edit = Some(pending);
                self.status = "Retrying edit with reserved tile capacity…".into();
            }
            Err(error) if error.code == ErrorCode::ResourceLimit => {} // Retain across worker-admission races.
            Err(error) => {
                self.waiting_edit = None;
                self.publication_preview = None;
                self.preview_queue_revision.advance();
                self.preview_lineage = Uuid::new_v4();
                self.status = format!("Edit not prepared: {error}");
            }
        }
    }
}

#[cfg(test)]
#[path = "retry/tests.rs"]
mod tests;
