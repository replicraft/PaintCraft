//! Atomic ingress transaction: never drain partially delivered input.
#[cfg(any(target_os = "macos", target_os = "windows", test))]
use std::sync::atomic::AtomicU64;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryHealth {
    Ready,
    /// A sink/source callback did not complete. Discard unpublished input and
    /// cancel the safe receiver's private stroke before releasing this handoff.
    Failed,
}
#[cfg(any(target_os = "macos", target_os = "windows", test))]
#[derive(Clone, Debug)]
pub(crate) struct Gate(Arc<State>);
#[derive(Debug)]
struct State {
    transaction: Mutex<()>,
    source_epoch: u64,
    failed: AtomicBool,
}
/// Sealed lifetime authority issued by an installed native adapter. Clones do
/// not keep capture active: installation retirement invalidates every clone.
#[derive(Clone, Debug)]
pub struct DeliveryCheck(Arc<State>);
impl DeliveryCheck {
    /// Authorize only records from this exact installed source. Invalid or
    /// foreign installation identifiers never invoke the supplied check.
    pub fn authorize_installation(
        &self,
        installation: u64,
        current: impl FnOnce() -> bool,
    ) -> bool {
        if installation == 0 || installation == u64::MAX || installation != self.0.source_epoch {
            return false;
        }
        self.authorize(current)
    }
    /// Authorize only a short context-free metadata check. Lock order is delivery
    /// then receiver ingress. No GUI, editor mutation, event pumping or Sink call
    /// may run here. The final successful failure read is the publication
    /// linearization point; document mutation follows after releasing both locks.
    pub fn authorize(&self, current: impl FnOnce() -> bool) -> bool {
        let _transaction = self.0.transaction.lock().unwrap_or_else(|e| e.into_inner());
        if self.0.failed.load(Ordering::Acquire) {
            return false;
        }
        let valid = std::panic::catch_unwind(std::panic::AssertUnwindSafe(current));
        if valid.is_err() {
            self.0.failed.store(true, Ordering::Release);
        }
        valid.unwrap_or(false) && !self.0.failed.load(Ordering::Acquire)
    }
}
#[cfg(any(target_os = "macos", target_os = "windows", test))]
#[derive(Debug)]
pub(crate) enum Failure {
    Source,
    Sink,
}
#[cfg(any(target_os = "macos", target_os = "windows", test))]
#[derive(Debug)]
pub(crate) enum Emission {
    Delivered(bool),
    FailedNow,
    FailedSource,
    Fenced,
}
#[cfg(any(target_os = "macos", target_os = "windows", test))]
impl Gate {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self::from_counter(&NEXT)
    }
    fn from_counter(counter: &AtomicU64) -> Self {
        let epoch = counter
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                n.checked_add(1).filter(|next| *next != u64::MAX)
            })
            .unwrap_or(0);
        Self(Arc::new(State {
            source_epoch: epoch,
            transaction: Mutex::new(()),
            failed: AtomicBool::new(epoch == 0),
        }))
    }
    pub(crate) fn source_epoch(&self) -> u64 {
        self.0.source_epoch
    }
    #[cfg(any(target_os = "macos", test))]
    pub(crate) fn emit(&self, sink: impl FnOnce() -> bool) -> Emission {
        self.emit_checked(|| Ok(sink()))
    }
    /// Source rejection and callback refusal are recorded before handoff can
    /// observe a partially delivered record. No gap exists after unlocking.
    pub(crate) fn emit_checked(&self, sink: impl FnOnce() -> Result<bool, Failure>) -> Emission {
        let _transaction = self.0.transaction.lock().unwrap_or_else(|e| e.into_inner());
        if self.0.failed.load(Ordering::Acquire) {
            return Emission::Fenced;
        }
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(sink)) {
            Ok(Ok(owned)) => {
                if self.0.failed.load(Ordering::Acquire) {
                    Emission::Fenced
                } else {
                    Emission::Delivered(owned)
                }
            }
            Ok(Err(source)) => {
                self.fail();
                match source {
                    Failure::Source => Emission::FailedSource,
                    Failure::Sink => Emission::FailedNow,
                }
            }
            Err(_) => {
                self.fail();
                Emission::FailedNow
            }
        }
    }
    pub(crate) fn check(&self) -> DeliveryCheck {
        DeliveryCheck(self.0.clone())
    }
    pub(crate) fn fail(&self) {
        // Callback reentrancy and installation Drop may fence while a transaction
        // is active. Never acquire the transaction lock on this failure path.
        self.0.failed.store(true, Ordering::Release);
    }
    pub(crate) fn with_delivery<T>(&self, take: impl FnOnce(DeliveryHealth) -> T) -> T {
        let _transaction = self.0.transaction.lock().unwrap_or_else(|e| e.into_inner());
        take(if self.0.failed.load(Ordering::Acquire) {
            DeliveryHealth::Failed
        } else {
            DeliveryHealth::Ready
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sealed_authorization_rechecks_reentrant_retirement() {
        let gate = Gate::new();
        let check = gate.check();
        assert!(check.authorize(|| true));
        assert!(!check.authorize(|| {
            gate.fail();
            true
        }));
        assert!(!check.authorize(|| panic!("retired check must not run")));
    }
    #[test]
    fn reentrant_retirement_during_sink_never_deadlocks_or_publishes() {
        let gate = Gate::new();
        assert!(matches!(
            gate.emit(|| {
                gate.fail();
                false
            }),
            Emission::Fenced
        ));
        assert_eq!(gate.with_delivery(|health| health), DeliveryHealth::Failed);
        assert!(!gate.check().authorize(|| true));
    }
    #[test]
    fn callback_panic_retires_previously_cloned_publication_authority() {
        let gate = Gate::new();
        let check = gate.check();
        assert!(matches!(
            gate.emit(|| panic!("partial delivery")),
            Emission::FailedNow
        ));
        assert!(!check.authorize(|| true));
    }
}

#[cfg(test)]
mod installation_tests {
    use super::*;
    #[test]
    fn exact_live_installation_is_required_before_callback() {
        let a = Gate::new();
        let b = Gate::new();
        let check = a.check();
        assert_ne!(a.source_epoch(), b.source_epoch());
        for wrong in [0, u64::MAX, b.source_epoch()] {
            assert!(!check.authorize_installation(wrong, || panic!("foreign callback")));
        }
        assert!(check.authorize_installation(a.source_epoch(), || true));
        a.fail();
        assert!(!check.authorize_installation(a.source_epoch(), || panic!("retired callback")));
    }
    #[test]
    fn exhausted_epoch_cannot_authorize_or_deliver() {
        let counter = AtomicU64::new(u64::MAX - 1);
        let gate = Gate::from_counter(&counter);
        assert_eq!(gate.source_epoch(), 0);
        assert_eq!(gate.with_delivery(|health| health), DeliveryHealth::Failed);
        assert!(
            !gate
                .check()
                .authorize_installation(0, || panic!("exhausted callback"))
        );
        assert!(matches!(
            gate.emit(|| panic!("exhausted sink")),
            Emission::Fenced
        ));
    }
}

/// Headless integration fixture, available only through the explicit dev feature.
/// Owns the real delivery gate; dropping it retires every cloned check.
#[cfg(all(
    feature = "test-support",
    any(target_os = "macos", target_os = "windows")
))]
#[derive(Debug)]
pub struct TestInstallation(Gate);
#[cfg(all(
    feature = "test-support",
    any(target_os = "macos", target_os = "windows")
))]
impl Default for TestInstallation {
    fn default() -> Self {
        Self(Gate::new())
    }
}
#[cfg(all(
    feature = "test-support",
    any(target_os = "macos", target_os = "windows")
))]
impl TestInstallation {
    pub fn source_epoch(&self) -> u64 {
        self.0.source_epoch()
    }
    pub fn delivery_check(&self) -> DeliveryCheck {
        self.0.check()
    }
}
#[cfg(all(
    feature = "test-support",
    any(target_os = "macos", target_os = "windows")
))]
impl Drop for TestInstallation {
    fn drop(&mut self) {
        self.0.fail();
    }
}
