//! Internal resource context. Wire error codes remain the stable public contract.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllocationClass {
    SharedBudget,
    DecodedTiles,
}
#[derive(Clone, Debug)]
pub struct AllocationDenial {
    budget: SharedBudget,
    class: AllocationClass,
    required: u64,
    observed_activity: u64,
}
impl AllocationDenial {
    pub(crate) fn new(budget: SharedBudget, required: u64) -> Self {
        let observed_activity = budget.activity();
        Self {
            budget,
            observed_activity,
            class: AllocationClass::SharedBudget,
            required,
        }
    }
    pub(crate) fn require(&mut self, class: AllocationClass, required: u64) {
        self.class = class;
        self.required = self.required.max(required);
    }
    pub(super) fn is_for(&self, budget: &SharedBudget) -> bool {
        std::sync::Arc::ptr_eq(&self.budget.0, &budget.0)
    }
    /// Atomically admit this known requirement in its original realm.
    pub fn reserve_capacity(&self) -> Result<BudgetReservation> {
        self.budget.reserve_capacity(self.required)
    }
    pub fn budget(&self) -> SharedBudget {
        self.budget.clone()
    }
    /// Repricing hint; denial/notification alone does not manufacture activity.
    pub fn activity_changed(&self) -> bool {
        let current = self.budget.activity();
        current == u64::MAX || current != self.observed_activity
    }
    pub fn class(&self) -> AllocationClass {
        self.class
    }
    pub fn required_bytes(&self) -> u64 {
        self.required
    }
    pub fn is_possible(&self) -> bool {
        self.required <= self.budget.usage().limit
    }
    /// A readiness hint for this exact realm; the next allocation still reserves.
    pub fn can_retry(&self) -> bool {
        self.budget.can_reserve(self.required)
    }
}
