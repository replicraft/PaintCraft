//! Sealed, context-free lifetime check retained by one native edit preparation.
use super::*;
#[derive(Clone, Debug)]
pub struct PublicationGuard {
    source: std::sync::Weak<Mutex<State>>,
    owner: Owner,
    cancel: u64,
    window: u64,
    canvas_epoch: u64,
    transform: Transform,
    canvas: egui::Rect,
    pixels_per_point: f64,
    canvas_samples: bool,
    pen_epoch: Option<u64>,
}
impl PublicationGuard {
    /// Metadata only. Native hosts call this inside their sealed delivery check,
    /// then release both locks before the editor commits its prepared result.
    pub fn is_current(&self) -> bool {
        let Some(source) = self.source.upgrade() else {
            return false;
        };
        let state = source.lock().unwrap_or_else(|e| e.into_inner());
        let Some(view) = state.view else {
            return false;
        };
        self.pen_epoch
            .is_none_or(|epoch| epoch != u64::MAX && epoch == state.pen_epoch)
            && self.cancel != u64::MAX
            && self.cancel == state.receiver.cancellation_epoch()
            && self.window != u64::MAX
            && self.window == state.window_epoch
            && view.owner == self.owner
            && view.focused
            && view.pixels_per_point == self.pixels_per_point
            && (!self.canvas_samples
                || (self.canvas_epoch != u64::MAX
                    && self.canvas_epoch == state.canvas_epoch
                    && !view.blocked
                    && !view.ime
                    && view.transform == self.transform
                    && view.canvas == self.canvas))
    }
}
impl Source {
    /// Bind a native editor publication to the opaque admitted batch, rather
    /// than the caller-mutable frame payload. No queue drain or allocation.
    pub fn publication_guard(&self, batch: &Batch, owner: Owner) -> Option<PublicationGuard> {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if !self.current_in(&state, batch, owner) {
            return None;
        }
        self.guard_in(&state, owner, batch.binding.canvas_samples, true)
    }
    /// Genuine source-observed GUI edges have no pen Batch. They still require
    /// the exact current document and installation/window lifetime at commit.
    pub fn ui_publication_guard(&self, owner: Owner) -> Option<PublicationGuard> {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        self.guard_in(&state, owner, false, false)
    }
    fn guard_in(
        &self,
        state: &State,
        owner: Owner,
        canvas_samples: bool,
        pen_bound: bool,
    ) -> Option<PublicationGuard> {
        let view = state.view?;
        if view.owner != owner || !view.focused {
            return None;
        }
        Some(PublicationGuard {
            source: Arc::downgrade(&self.0),
            owner,
            cancel: state.receiver.cancellation_epoch(),
            window: state.window_epoch,
            canvas_epoch: state.canvas_epoch,
            transform: view.transform,
            canvas: view.canvas,
            pixels_per_point: view.pixels_per_point,
            canvas_samples,
            pen_epoch: pen_bound.then_some(state.pen_epoch),
        })
    }
}
