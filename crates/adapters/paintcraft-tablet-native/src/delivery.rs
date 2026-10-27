//! Atomic ingress transaction: never drain partially delivered input.
#[cfg(any(target_os = "macos", test))]
use std::sync::{Arc, Mutex};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryHealth {
    Ready,
    /// A sink/source callback did not complete. Discard unpublished input and
    /// cancel the safe receiver's private stroke before releasing this handoff.
    Failed,
}
#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Debug)]
pub(crate) struct Gate(Arc<Mutex<DeliveryHealth>>);
#[cfg(any(target_os = "macos", test))]
#[derive(Debug)]
pub(crate) enum Failure {
    Source,
    Sink,
}
#[cfg(any(target_os = "macos", test))]
#[derive(Debug)]
pub(crate) enum Emission {
    Delivered(bool),
    FailedNow,
    FailedSource,
    Fenced,
}
#[cfg(any(target_os = "macos", test))]
impl Gate {
    pub(crate) fn new() -> Self {
        Self(Arc::new(Mutex::new(DeliveryHealth::Ready)))
    }
    pub(crate) fn emit(&self, sink: impl FnOnce() -> bool) -> Emission {
        self.emit_checked(|| Ok(sink()))
    }
    /// Source rejection and callback refusal are recorded before handoff can
    /// observe a partially delivered record. No gap exists after unlocking.
    pub(crate) fn emit_checked(&self, sink: impl FnOnce() -> Result<bool, Failure>) -> Emission {
        let mut health = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if *health == DeliveryHealth::Failed {
            return Emission::Fenced;
        }
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(sink)) {
            Ok(Ok(owned)) => Emission::Delivered(owned),
            Ok(Err(source)) => {
                *health = DeliveryHealth::Failed;
                match source {
                    Failure::Source => Emission::FailedSource,
                    Failure::Sink => Emission::FailedNow,
                }
            }
            Err(_) => {
                *health = DeliveryHealth::Failed;
                Emission::FailedNow
            }
        }
    }
    pub(crate) fn fail(&self) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = DeliveryHealth::Failed;
    }
    pub(crate) fn with_delivery<T>(&self, take: impl FnOnce(DeliveryHealth) -> T) -> T {
        let health = self.0.lock().unwrap_or_else(|e| e.into_inner());
        take(*health)
    }
}
