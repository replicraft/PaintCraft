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
    Paste(
        Arc<paintcraft_editor::clipboard::PasteDraft>,
        PasteDestination,
    ),
    Navigation(background::NavigationRequest),
}
/// Frozen placement policy retained unchanged across admission retries.
#[derive(Clone, Debug)]
pub(super) enum PasteDestination {
    NewLayer(String),
    ExistingLayer {
        target: Uuid,
        blend: paintcraft_core::types::policy::BlendMode,
        key: Option<Rgba>,
    },
}
#[derive(Clone, Debug)]
struct Realms {
    tiles: paintcraft_core::types::resources::SharedBudget,
    metadata: paintcraft_core::types::resources::SharedBudget,
}
impl Realms {
    fn capture(admission: &Admission) -> Self {
        Self {
            tiles: admission.residency_budget(),
            metadata: admission.paint_metadata_budget(),
        }
    }
    fn admission(&self) -> Admission {
        Admission::with_residency(u64::MAX, self.tiles.clone())
            .with_paint_metadata_budget(self.metadata.clone())
    }
    fn metadata_retry(&self, denial: &AllocationDenial, reprice: bool) -> Option<Admission> {
        match denial.reserve_capacity() {
            Ok(reservation) => Some(self.admission().with_reserved_paint_metadata(reservation)),
            // A bounded periodic live probe can discover changed demand. It owns
            // no reservation and cannot turn its own cleanup into an eager loop.
            Err(_) if reprice => Some(self.admission()),
            Err(_) => None,
        }
    }
}
#[derive(Debug)]
pub(super) struct Deferred {
    request: Request,
    base: HistoryTicket,
    denial: AllocationDenial,
    realms: Realms,
}
#[derive(Debug)]
pub(super) struct Waiting {
    pub(super) text: Option<text_submission::Attempt>,
    pub(super) native_activation: Option<native_publication::Scope>,
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
    let realms = Realms::capture(&admission);
    let result = preparation
        .apply_with_admission(&batch, admission, &check)
        .map(Completion::Edit);
    defer_failure(base, Request::Edit(batch), result, realms, check)
}
pub(super) fn prepare_paste(
    preparation: Preparation,
    draft: Arc<paintcraft_editor::clipboard::PasteDraft>,
    destination: PasteDestination,
    admission: Admission,
    check: impl Fn() -> Result<()>,
) -> Result<Completion> {
    let base = preparation.history_ticket();
    let realms = Realms::capture(&admission);
    let result = match &destination {
        PasteDestination::NewLayer(name) => {
            preparation.paste_image(&draft, name, admission, &check)
        }
        PasteDestination::ExistingLayer { target, blend, key } => {
            preparation.paste_image_onto(&draft, *target, *blend, *key, admission, &check)
        }
    }
    .map(Completion::Edit);
    defer_failure(
        base,
        Request::Paste(draft, destination),
        result,
        realms,
        check,
    )
}
pub(super) fn prepare_navigation(
    preparation: Preparation,
    request: background::NavigationRequest,
    admission: Admission,
    check: impl Fn() -> Result<()>,
) -> Result<Completion> {
    let base = preparation.history_ticket();
    let realms = Realms::capture(&admission);
    let result = match request {
        background::NavigationRequest::Undo => preparation.undo_with_admission(admission, &check),
        background::NavigationRequest::Redo => preparation.redo_with_admission(admission, &check),
        background::NavigationRequest::Seek { ticket, history_id } => {
            preparation.navigate_history_with_admission(ticket, history_id, admission, &check)
        }
    }
    .map(Completion::Navigation);
    defer_failure(base, Request::Navigation(request), result, realms, check)
}
fn defer_failure(
    base: HistoryTicket,
    request: Request,
    result: Result<Completion>,
    realms: Realms,
    check: impl Fn() -> Result<()>,
) -> Result<Completion> {
    match result {
        Ok(completion) => Ok(completion),
        Err(error) => {
            check()?;
            match error.allocation_denial().filter(|denial| {
                matches!(
                    denial.class(),
                    AllocationClass::DecodedTiles | AllocationClass::PaintMetadata
                ) && denial.is_possible()
            }) {
                Some(denial) => Ok(Completion::Deferred(Box::new(Deferred {
                    request,
                    base,
                    denial: denial.clone(),
                    realms,
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
            .filter(|denial| denial.class() == AllocationClass::DecodedTiles)
    }
    pub(crate) fn metadata_wait(&self) -> Option<&AllocationDenial> {
        self.waiting_edit
            .as_ref()
            .filter(|waiting| waiting.deferred.base == self.session.history_ticket())
            .map(|waiting| &waiting.deferred.denial)
            .filter(|denial| denial.class() == AllocationClass::PaintMetadata)
    }
    pub(super) fn retry_waiting_edit(&mut self) {
        let Some(waiting) = &self.waiting_edit else {
            return;
        };
        if waiting.deferred.base != self.session.history_ticket()
            || self.check_text_attempt(waiting.text).is_err()
        {
            self.fail_text_attempt(waiting.text);
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
        let metadata = waiting.deferred.denial.class() == AllocationClass::PaintMetadata;
        let reprice = (metadata
            && waiting.last_probe.elapsed() >= std::time::Duration::from_secs(5))
            || (!metadata && navigation)
                && (waiting.deferred.denial.activity_changed()
                    || waiting.last_probe.elapsed() >= std::time::Duration::from_secs(5));
        if !waiting.deferred.denial.can_retry() && !reprice {
            return;
        }
        let admission = if metadata {
            let Some(admission) = waiting
                .deferred
                .realms
                .metadata_retry(&waiting.deferred.denial, reprice)
            else {
                return;
            };
            admission
        } else if navigation {
            // The worker pins current reusable versions and reserves the entire
            // missing set, rather than trusting the previous attempt's demand.
            Admission::with_residency(u64::MAX, waiting.deferred.denial.budget())
                .with_paint_metadata_budget(waiting.deferred.realms.metadata.clone())
        } else {
            let Ok(reservation) = waiting.deferred.denial.reserve_capacity() else {
                return;
            };
            Admission::with_reserved_residency(u64::MAX, reservation)
                .with_paint_metadata_budget(waiting.deferred.realms.metadata.clone())
        };
        let text = waiting.text;
        let preparation = match self.session.preparation() {
            Ok(preparation) => preparation,
            Err(error) => {
                self.fail_text_attempt(text);
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
                    Request::Paste(draft, destination) => {
                        prepare_paste(preparation, draft, destination, admission, check)
                    }
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
            Ok(mut pending) => {
                pending.native_activation = waiting.native_activation.clone();
                pending.text = text;
                self.waiting_edit = None;
                self.pending_edit = Some(pending);
                self.status = "Retrying edit after memory capacity became available…".into();
            }
            Err(error) if error.code == ErrorCode::ResourceLimit => {} // Retain across worker-admission races.
            Err(error) => {
                self.fail_text_attempt(text);
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

#[cfg(test)]
#[path = "retry/metadata_tests.rs"]
mod metadata_tests;

#[cfg(test)]
#[path = "retry/paste_tests.rs"]
mod paste_tests;
