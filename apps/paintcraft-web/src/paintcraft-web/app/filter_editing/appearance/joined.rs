//! Unknown remote retirement cannot release local scratch or broker Main credit.
//! One admitted slot bounds this quarantine. It persists until page teardown;
//! RPC rejection is not evidence that the Worker released its captured owner.
#[derive(Debug)]
pub(super) struct Joined7<T> {
    owner: Option<T>,
}
impl<T> Joined7<T> {
    pub fn new(owner: T) -> Self {
        Self { owner: Some(owner) }
    }
    pub fn acknowledged(mut self) {
        drop(self.owner.take());
    }
}
impl<T> Drop for Joined7<T> {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            // Deliberately retain the exact owner, including credit/scope, on
            // failed close, bad ACK, or interrupted join. The existing broker
            // also remains blocked/quarantined. No fresh slot is minted.
            std::mem::forget(owner);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use paintcraft_core::types::resources::{AllocationClass, SharedBudget};
    #[test]
    fn pending_open_and_close_keep_actual_account_until_acknowledged() {
        let budget = SharedBudget::new(1024);
        let mut account = budget.account(AllocationClass::SharedBudget);
        account.resize(64).unwrap();
        let joining = Joined7::new(account);
        assert_eq!(budget.usage().reserved, 64); // OPEN failed, close still pending
        joining.acknowledged();
        assert_eq!(budget.usage().reserved, 0);
    }
    #[test]
    fn rejected_bad_ack_and_interrupted_join_quarantine_actual_account_instead_of_readmitting() {
        // These tiny owners intentionally remain quarantined for this test
        // process lifetime, as they do until actual browser page teardown.
        for _failure in ["close rejection", "bad ACK", "future dropped"] {
            let budget = SharedBudget::new(64);
            let mut account = budget.account(AllocationClass::SharedBudget);
            account.resize(64).unwrap();
            drop(Joined7::new(account));
            assert_eq!(budget.usage().reserved, 64);
            assert!(budget.reserve_capacity(1).is_err());
        }
    }
}
