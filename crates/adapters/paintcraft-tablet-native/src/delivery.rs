//! Atomic ingress transaction: never drain partially delivered input.
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
    failed: AtomicBool,
}
/// Sealed lifetime authority issued by an installed native adapter. Clones do
/// not keep capture active: installation retirement invalidates every clone.
#[derive(Clone, Debug)]
pub struct DeliveryCheck(Arc<State>);
impl DeliveryCheck {
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
        Self(Arc::new(State {
            transaction: Mutex::new(()),
            failed: AtomicBool::new(false),
        }))
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
