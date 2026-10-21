//! Process-local GPU lifetime identity. Invalidated epochs never become current again.
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug)]
pub struct DeviceEpoch(Arc<AtomicBool>);
impl Default for DeviceEpoch {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }
}
impl PartialEq for DeviceEpoch {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for DeviceEpoch {}
impl DeviceEpoch {
    pub fn is_current(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    /// Called by the device owner on loss/reset; does not touch editor state.
    pub fn retire(&self) {
        self.0.store(false, Ordering::Release);
    }
    pub fn check(&self) -> Result<()> {
        if self.is_current() {
            Ok(())
        } else {
            Err(Error::new(
                ErrorCode::CapabilityUnsupported,
                "GPU device epoch has ended",
            ))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retirement_is_shared_irreversible_and_cannot_alias_a_replacement() {
        let old = DeviceEpoch::default();
        let reader = old.clone();
        let replacement = DeviceEpoch::default();
        assert_eq!(old, reader);
        assert_ne!(old, replacement);
        std::thread::spawn(move || reader.retire()).join().unwrap();
        assert!(!old.is_current());
        assert!(old.check().is_err());
        old.retire();
        assert!(replacement.check().is_ok());
    }
}
