//! Native touch admission only. No egui events, tool actions or pointer promotion.
use super::*;
use paintcraft_input::tablet::{FingerConfidence, Identity, Provenance, Time};
use std::sync::atomic::{AtomicBool, Ordering};
mod resident;
mod state;
pub(super) use resident::Resident;
use state::Change;
pub(super) use state::Metadata;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeFingerMetadata {
    pub installation: u64,
    pub ordinal: u64,
    pub identity: Identity,
    pub phase: Phase,
    pub time: Time,
    pub provenance: Provenance,
    pub confidence: FingerConfidence,
    pub in_contact: bool,
    pub primary: bool,
}
/// Candidate is not positive authority: Source independently checks lifetime,
/// context, real contact edges and competing physical captures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeFingerAdmission {
    Candidate,
    Suppressed,
    PolicyCancel,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeTouchTarget {
    Canvas,
    Ui,
}
#[derive(Clone, Copy, Debug)]
struct CaptureContext {
    view: View,
    cancel: u64,
    window: u64,
    canvas: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key {
    installation: u64,
    identity: Identity,
    down: u64,
}
#[derive(Debug)]
struct Cell {
    alive: AtomicBool,
    _storage: BudgetAccount,
}
#[derive(Clone, Debug)]
pub(super) struct Lease {
    key: Key,
    valid: Arc<Cell>,
}
impl Lease {
    pub(super) fn current(&self) -> bool {
        self.valid.alive.load(Ordering::Acquire)
    }
}
/// Immutable admitted source data and opaque context/lifetime binding. A caller
/// cannot turn changed metadata into a different admission.
#[derive(Debug)]
pub struct NativeTouchBatch {
    metadata: NativeFingerMetadata,
    position: Option<(f64, f64)>,
    target: NativeTouchTarget,
    primary: bool,
    pub(super) binding: Binding,
    _storage: BudgetAccount,
}
impl NativeTouchBatch {
    pub fn metadata(&self) -> NativeFingerMetadata {
        self.metadata
    }
    pub fn position(&self) -> Option<(f64, f64)> {
        self.position
    }
    pub fn target(&self) -> NativeTouchTarget {
        self.target
    }
    /// Actual system-primary fact captured on this lifetime's Down. Terminal
    /// raw flags remain separately available in metadata(); never reassign it.
    pub fn captured_primary(&self) -> bool {
        self.primary
    }
    pub fn down_ordinal(&self) -> u64 {
        self.binding
            .native_touch
            .as_ref()
            .expect("native touch binding")
            .key
            .down
    }
}
/// A presentation-only retirement, never a fabricated physical Up/Cancel or an
/// authorization to run widgets. `cause=None` is an explicit host lifecycle action.
#[derive(Clone, Debug)]
pub struct NativeTouchCancellation {
    lease: Lease,
    cause: Option<NativeFingerMetadata>,
    position: Option<(f64, f64)>,
    target: NativeTouchTarget,
    primary: bool,
}
impl NativeTouchCancellation {
    /// Exact private cell identity, not equality of caller-visible tuples.
    pub fn matches_batch(&self, batch: &NativeTouchBatch) -> bool {
        batch
            .binding
            .native_touch
            .as_ref()
            .is_some_and(|lease| self.same_lease(lease))
    }
    pub fn matches_publication(&self, guard: &PublicationGuard) -> bool {
        guard
            .native_touch
            .as_ref()
            .is_some_and(|lease| self.same_lease(lease))
    }
    fn same_lease(&self, lease: &Lease) -> bool {
        self.lease.key == lease.key && Arc::ptr_eq(&self.lease.valid, &lease.valid)
    }
    pub fn captured_primary(&self) -> bool {
        self.primary
    }
    pub fn identity(&self) -> Identity {
        self.lease.key.identity
    }
    pub fn installation(&self) -> u64 {
        self.lease.key.installation
    }
    pub fn down_ordinal(&self) -> u64 {
        self.lease.key.down
    }
    pub fn cause(&self) -> Option<NativeFingerMetadata> {
        self.cause
    }
    pub fn last_observed_position(&self) -> Option<(f64, f64)> {
        self.position
    }
    pub fn target(&self) -> NativeTouchTarget {
        self.target
    }
}
// Keep the charged fixed-size batch inline; no callback-time Box allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum NativeTouchOutcome {
    Suppressed,
    Admitted(NativeTouchBatch),
    Cancelled(NativeTouchCancellation),
}
impl Source {
    /// Observe metadata even without a layout. Conversion only runs for usable
    /// candidate records under the same source lock. It must not touch Context,
    /// the editor, native APIs or a delivery gate.
    pub fn submit_native_finger(
        &self,
        metadata: NativeFingerMetadata,
        admission: NativeFingerAdmission,
        convert: impl FnOnce(f64) -> std::result::Result<Option<(f64, f64)>, Invalid>,
    ) -> std::result::Result<NativeTouchOutcome, Invalid> {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let context = state.view.map(|view| CaptureContext {
            view,
            cancel: state.receiver.cancellation_epoch(),
            window: state.window_epoch,
            canvas: state.canvas_epoch,
        });
        let capture = state.receiver.capture_status();
        let change = state
            .native_touch
            .observe(metadata, admission, context, capture, convert);
        let change = match change {
            Ok(change) => change,
            Err(error) => {
                state.native_touch.deny(metadata.identity);
                if error == Invalid::Capacity {
                    // Actual acquisition capacity failure retains the existing
                    // global-fault policy; this is not a pen/touch-only cancel.
                    state.native_touch.fence_all();
                    state.native.fence();
                    state.receiver.fault();
                }
                return Err(error);
            }
        };
        match change {
            Change::Suppressed => Ok(NativeTouchOutcome::Suppressed),
            Change::Cancelled(cancel) => Ok(NativeTouchOutcome::Cancelled(cancel)),
            Change::Admitted {
                lease,
                position,
                target,
                primary,
                context,
            } => {
                let view = context.view;
                let mut storage = state.native_touch.frame_account()?;
                if storage
                    .resize(std::mem::size_of::<NativeTouchBatch>() as u64)
                    .is_err()
                {
                    state.native_touch.fence_all();
                    state.native.fence();
                    state.receiver.fault();
                    return Err(Invalid::Capacity);
                }
                let down = lease.key.down;
                Ok(NativeTouchOutcome::Admitted(NativeTouchBatch {
                    metadata,
                    position,
                    target,
                    primary,
                    binding: Binding {
                        source: Arc::downgrade(&self.0),
                        owner: view.owner,
                        view: Some(view),
                        cancel_epoch: context.cancel,
                        window_epoch: context.window,
                        canvas_epoch: context.canvas,
                        pen_epoch: None,
                        native_touch: Some(lease),
                        native_contacts: None,
                        acknowledged: None,
                        canvas_samples: target == NativeTouchTarget::Canvas,
                        last_down: Some(down),
                    },
                    _storage: storage,
                }))
            }
        }
    }
    pub fn native_touch_batch_is_current(&self, batch: &NativeTouchBatch, owner: Owner) -> bool {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        self.current_binding(&state, &batch.binding, owner)
    }
    pub fn native_touch_publication_guard(
        &self,
        batch: &NativeTouchBatch,
        owner: Owner,
    ) -> Option<PublicationGuard> {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if !self.current_binding(&state, &batch.binding, owner) {
            return None;
        }
        let mut guard = self.guard_in(&state, owner, batch.binding.canvas_samples, false)?;
        guard.native_touch = batch.binding.native_touch.clone();
        Some(guard)
    }
    /// Touch-only retirement includes drained/completed pending lifetimes and
    /// never invalidates genuine mouse or pen publication authority. Consumers
    /// use these opaque keys solely to retire matching private presentation/jobs.
    pub fn retire_native_touches(
        &self,
    ) -> std::result::Result<[Option<NativeTouchCancellation>; 32], Invalid> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .native_touch
            .retire_all()
    }
    /// Retire only qualified, physically held touch lifetimes. Earlier completed
    /// batches, pending edits and genuine mouse publication guards stay valid.
    /// The returned descriptors grant only exact negative presentation authority.
    pub fn retire_active_native_touches(
        &self,
    ) -> std::result::Result<[Option<NativeTouchCancellation>; 32], Invalid> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .native_touch
            .retire_active()
    }
    /// Only after Collector verified a true native !INRANGE/no-held lifetime
    /// end. Ordinary Leave/Up/cancel is not clock reclamation evidence.
    pub fn retire_native_touch_identity(
        &self,
        identity: Identity,
        installation: u64,
        ordinal: u64,
        time: Time,
    ) -> std::result::Result<(), Invalid> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .native_touch
            .retire_identity(identity, installation, ordinal, time)
    }
}
#[cfg(test)]
mod tests;
