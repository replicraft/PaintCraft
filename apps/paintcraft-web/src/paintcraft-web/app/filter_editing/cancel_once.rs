//! One Main job forwards cancellation once, even while its first ACK is pending.
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Debug, Default)]
pub(super) struct CancelOnce(Rc<Cell<bool>>);

impl CancelOnce {
    pub(super) fn run_once(&self, forward: impl FnOnce()) {
        // Claim before invoking external JavaScript: reentry cannot forward a
        // duplicate request, and a denied request cannot be retried every frame.
        if !self.0.replace(true) {
            forward();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloned_pending_job_forwards_one_cancel_and_never_resets_after_ack() {
        let guard = CancelOnce::default();
        let copies: Vec<_> = (0..32).map(|_| guard.clone()).collect();
        let forwarded = Cell::new(0);
        let pending = Cell::new(false);
        for copy in &copies {
            for _ in 0..1000 {
                copy.run_once(|| {
                    forwarded.set(forwarded.get() + 1);
                    pending.set(true);
                });
            }
        }
        assert!(pending.get());
        assert_eq!(forwarded.get(), 1);
        pending.set(false);
        guard.run_once(|| forwarded.set(forwarded.get() + 1));
        assert_eq!(forwarded.get(), 1);
        // A separate output job has its own cancellation lifetime.
        CancelOnce::default().run_once(|| forwarded.set(forwarded.get() + 1));
        assert_eq!(forwarded.get(), 2);
    }

    #[test]
    fn claim_precedes_external_reentry() {
        let guard = CancelOnce::default();
        let same = guard.clone();
        let forwarded = Cell::new(0);
        guard.run_once(|| {
            forwarded.set(forwarded.get() + 1);
            same.run_once(|| forwarded.set(forwarded.get() + 1));
        });
        assert_eq!(forwarded.get(), 1);
    }
}
