//! Retain the monitor/proximity cache while recovering lost mouse capture.
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Debug, Default)]
pub(super) struct Recovery(AtomicBool);
impl Recovery {
    pub(super) fn focus_lost(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub(super) fn observe_idle(&self, focused: bool, combined_buttons: usize) -> bool {
        // Never subtract pen bits to guess physical mouse state. A first pen
        // activation without a prior idle observation remains unresolved.
        focused && combined_buttons == 0 && self.0.swap(false, Ordering::AcqRel)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_actual_focused_idle_releases_recovery_once() {
        let r = Recovery::default();
        assert!(!r.observe_idle(true, 0));
        r.focus_lost();
        assert!(!r.observe_idle(false, 0));
        assert!(!r.observe_idle(true, 1));
        assert!(!r.observe_idle(true, 1 << 4));
        assert!(r.observe_idle(true, 0));
        assert!(!r.observe_idle(true, 0));
    }
}
