//! Pure defining-type facts for explicit downstream factory declarations.
use super::{BudgetReservation, BudgetScope, Error, ErrorCode, Result, SharedBudget, State};
use std::{
    mem::{align_of, size_of},
    sync::Mutex,
};

fn arc_bytes<T>() -> Result<u64> {
    let error = || Error::new(ErrorCode::ResourceLimit, "factory control layout");
    let align = align_of::<T>().max(align_of::<usize>());
    let offset = (2 * size_of::<usize>())
        .checked_next_multiple_of(align_of::<T>())
        .ok_or_else(error)?;
    offset
        .checked_add(size_of::<T>())
        .and_then(|n| n.checked_next_multiple_of(align))
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(error)
}

impl SharedBudget {
    /// Logical inline `Arc<Mutex<State>>` allocation for one fresh budget.
    ///
    /// This is a pure defining-type fact, not a grant. It creates no budget,
    /// reservation, scope, observer or counter. Charge the quote before the real
    /// constructor wherever the containing host's bootstrap contract permits.
    /// Allocator overhead/slack, RSS, observer storage and lazy OS mutex backing
    /// allocations are excluded. In particular, macOS may allocate pthread
    /// backing separately when a mutex is first used.
    pub fn control_allocation_quote() -> Result<u64> {
        arc_bytes::<Mutex<State>>()
    }
}
impl BudgetReservation {
    /// Logical inline `Arc<Reserved>` allocation for one fresh reservation.
    ///
    /// The reserved payload capacity and inline returned handle are separate.
    /// Cloning a reservation reuses this allocation. This quote creates no
    /// reservation or authority and excludes allocator overhead/slack, RSS and
    /// lazy OS mutex backing allocations.
    pub fn control_allocation_quote() -> Result<u64> {
        arc_bytes::<super::reservation::Reserved>()
    }
}
impl BudgetScope {
    /// Logical inline `Arc<Mutex<ScopeFootprint>>` for one fresh scope state.
    ///
    /// Existing budget/reservation allocations and the inline returned handle
    /// are separate. Cloning a scope reuses its state. This quote creates no
    /// scope or authority and excludes allocator overhead/slack, RSS and lazy
    /// OS mutex backing allocations.
    pub fn control_allocation_quote() -> Result<u64> {
        arc_bytes::<Mutex<super::ScopeFootprint>>()
    }
}

#[cfg(test)]
#[path = "factory_control_tests.rs"]
mod tests;
