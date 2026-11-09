//! Primitive teardown qualification, separate from whole-source domain tests.
use super::*;
use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
use std::sync::{
    Barrier,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
struct Payload(Arc<AtomicUsize>);
impl Drop for Payload {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
struct Pair {
    payload: PrivateArc<Payload>,
    credit: PrivateArc<BudgetAccount>,
}
impl Pair {
    fn clone_pair(&self) -> Self {
        Self {
            payload: PrivateArc(self.payload.0.clone()),
            credit: PrivateArc(self.credit.0.clone()),
        }
    }
}
impl Drop for Pair {
    fn drop(&mut self) {
        self.payload.clear();
        self.credit.clear();
    }
}
#[test]
fn nonfinal_owner_cannot_release_payload_or_paired_credit() {
    let budget = SharedBudget::new(128);
    let mut account = budget.account(AllocationClass::SharedBudget);
    account.resize(128).unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let first = Pair {
        payload: PrivateArc::new(Payload(hits.clone())),
        credit: PrivateArc::new(account),
    };
    let second = first.clone_pair();
    drop(first);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    assert_eq!(budget.usage().reserved, 128);
    drop(second);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert_eq!(budget.usage().reserved, 0);
}
#[test]
fn concurrent_final_owners_finish_payload_before_credit_notifies_reentry() {
    let budget = SharedBudget::new(128);
    let mut account = budget.account(AllocationClass::SharedBudget);
    account.resize(128).unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = Arc::new(AtomicBool::new(false));
    let check_hits = hits.clone();
    let check_observed = observed.clone();
    let watch = budget
        .watch(1, move || {
            check_observed.store(check_hits.load(Ordering::SeqCst) == 1, Ordering::SeqCst);
        })
        .unwrap();
    watch.observe();
    let first = Pair {
        payload: PrivateArc::new(Payload(hits.clone())),
        credit: PrivateArc::new(account),
    };
    let second = first.clone_pair();
    let barrier = Arc::new(Barrier::new(3));
    std::thread::scope(|scope| {
        let a = barrier.clone();
        let first = scope.spawn(move || {
            a.wait();
            drop(first);
        });
        let b = barrier.clone();
        let second = scope.spawn(move || {
            b.wait();
            drop(second);
        });
        barrier.wait();
        first.join().unwrap();
        second.join().unwrap();
    });
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(observed.load(Ordering::SeqCst));
    assert_eq!(budget.usage().reserved, 0);
    // No Weak diagnostic is retained: exact Arc-block-before-value ordering is
    // the selected into_inner source contract, not inferred from an RSS sample.
}
