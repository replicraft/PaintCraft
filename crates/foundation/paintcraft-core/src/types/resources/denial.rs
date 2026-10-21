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
}
impl AllocationDenial {
    pub(crate) fn new(budget: SharedBudget, required: u64) -> Self {
        Self {
            budget,
            class: AllocationClass::SharedBudget,
            required,
        }
    }
    pub(crate) fn require(&mut self, class: AllocationClass, required: u64) {
        self.class = class;
        self.required = self.required.max(required);
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
