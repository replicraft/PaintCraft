//! Keep an admitted edit at its FIFO position while decoded capacity is unavailable.
use super::*;
use background::Completion;
use paintcraft_core::types::{
    ErrorCode,
    resources::{AllocationClass, AllocationDenial},
};
use paintcraft_editor::session::{HistoryTicket, Preparation};
use paintcraft_raster::Admission;

#[derive(Clone, Debug)]
enum Request {
    Edit(Arc<Batch>),
    Navigation(background::NavigationRequest),
}
#[derive(Debug)]
pub(super) struct Deferred {
    request: Request,
    base: HistoryTicket,
    denial: AllocationDenial,
}
#[derive(Debug)]
pub(super) struct Waiting {
    pub(super) deferred: Deferred,
    pub(super) done: String,
    pub(super) last_probe: std::time::Instant,
    pub(super) empty: &'static str,
    pub(super) target_alias: Option<&'static str>,
}
pub(super) fn prepare_batch(
    preparation: Preparation,
    batch: Arc<Batch>,
    admission: Admission,
    check: impl Fn() -> Result<()>,
) -> Result<Completion> {
    let base = preparation.history_ticket();
    let result = preparation
        .apply_with_admission(&batch, admission, &check)
        .map(Completion::Edit);
    defer_failure(base, Request::Edit(batch), result, check)
}
pub(super) fn prepare_navigation(
    preparation: Preparation,
    request: background::NavigationRequest,
    admission: Admission,
    check: impl Fn() -> Result<()>,
) -> Result<Completion> {
    let base = preparation.history_ticket();
    let result = match request {
        background::NavigationRequest::Undo => preparation.undo_with_admission(admission, &check),
        background::NavigationRequest::Redo => preparation.redo_with_admission(admission, &check),
        background::NavigationRequest::Seek { ticket, history_id } => {
            preparation.navigate_history_with_admission(ticket, history_id, admission, &check)
        }
    }
    .map(Completion::Navigation);
    defer_failure(base, Request::Navigation(request), result, check)
}
fn defer_failure(
    base: HistoryTicket,
    request: Request,
    result: Result<Completion>,
    check: impl Fn() -> Result<()>,
) -> Result<Completion> {
    match result {
        Ok(completion) => Ok(completion),
        Err(error) => {
            check()?;
            match error.allocation_denial().filter(|denial| {
                denial.class() == AllocationClass::DecodedTiles && denial.is_possible()
            }) {
                Some(denial) => Ok(Completion::Deferred(Box::new(Deferred {
                    request,
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
        let navigation = matches!(waiting.deferred.request, Request::Navigation(_));
        let reprice = navigation
            && (waiting.deferred.denial.activity_changed()
                || waiting.last_probe.elapsed() >= std::time::Duration::from_secs(5));
        if !waiting.deferred.denial.can_retry() && !reprice {
            return;
        }
        let admission = if navigation {
            // The worker pins current reusable versions and reserves the entire
            // missing set, rather than trusting the previous attempt's demand.
            Admission::with_residency(u64::MAX, waiting.deferred.denial.budget())
        } else {
            let Ok(reservation) = waiting.deferred.denial.reserve_capacity() else {
                return;
            };
            Admission::with_reserved_residency(u64::MAX, reservation)
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
        let request = waiting.deferred.request.clone();
        let result = service.submit(
            move |cancellation| {
                let check = || cancellation.check();
                match request {
                    Request::Edit(batch) => prepare_batch(preparation, batch, admission, check),
                    Request::Navigation(request) => {
                        prepare_navigation(preparation, request, admission, check)
                    }
                }
            },
            waiting.done.clone(),
            waiting.empty,
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

#[cfg(test)]
#[path = "retry/navigation_tests.rs"]
mod navigation_tests;
