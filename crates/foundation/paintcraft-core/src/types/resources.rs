//! Shared byte reservations with ownership-based release.
//! Consumers define the charged resource; this is not allocator/RSS interception.
use super::{Error, ErrorCode, Result};
use std::sync::{Arc, Mutex};
mod watch;
pub use watch::{BudgetPressure, BudgetWatch};
mod reservation;
pub use reservation::BudgetReservation;
mod denial;
pub use denial::{AllocationClass, AllocationDenial};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BudgetUsage {
    pub limit: u64,
    pub reserved: u64,
    pub peak: u64,
}
#[derive(Debug)]
struct State {
    limit: u64,
    reserved: u64,
    peak: u64,
    observers: watch::Registrations,
}
#[derive(Clone, Debug)]
pub struct SharedBudget(Arc<Mutex<State>>);
impl SharedBudget {
    pub fn new(limit: u64) -> Self {
        Self(Arc::new(Mutex::new(State {
            limit,
            reserved: 0,
            peak: 0,
            observers: Vec::new(),
        })))
    }
    pub fn usage(&self) -> BudgetUsage {
        let state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        BudgetUsage {
            limit: state.limit,
            reserved: state.reserved,
            peak: state.peak,
        }
    }
    pub fn can_reserve(&self, bytes: u64) -> bool {
        let usage = self.usage();
        bytes <= usage.limit - usage.reserved
    }
    pub fn reserve(&self, bytes: u64) -> Result<BudgetLease> {
        self.admit(bytes)?;
        Ok(BudgetLease {
            _charge: Arc::new(Charge {
                budget: self.clone(),
                bytes,
                reservation: None,
            }),
        })
    }
    fn admit(&self, bytes: u64) -> Result<()> {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let before = state.limit - state.reserved;
        let reserved = state
            .reserved
            .checked_add(bytes)
            .filter(|total| *total <= state.limit);
        let Some(reserved) = reserved else {
            let feasible = bytes <= state.limit;
            let wakes = watch::collect(&mut state, before, feasible, false);
            drop(state);
            watch::notify(wakes);
            return Err(Error::denied(self.clone(), bytes));
        };
        state.reserved = reserved;
        state.peak = state.peak.max(reserved);
        let wakes = watch::collect(&mut state, before, false, false);
        drop(state);
        watch::notify(wakes);
        Ok(())
    }
    fn release(&self, bytes: u64) {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let before = state.limit - state.reserved;
        state.reserved -= bytes;
        let wakes = watch::collect(&mut state, before, false, bytes != 0);
        drop(state);
        watch::notify(wakes);
    }
}
#[derive(Debug)]
struct Charge {
    budget: SharedBudget,
    bytes: u64,
    reservation: Option<std::sync::Weak<reservation::Reserved>>,
}
impl Drop for Charge {
    fn drop(&mut self) {
        if let Some(reservation) = self.reservation.as_ref().and_then(std::sync::Weak::upgrade) {
            *reservation
                .available
                .lock()
                .unwrap_or_else(|error| error.into_inner()) += self.bytes;
        } else {
            self.budget.release(self.bytes);
        }
    }
}
#[must_use = "retain the lease until the charged resource has been released"]
#[derive(Clone, Debug)]
pub struct BudgetLease {
    _charge: Arc<Charge>,
}

#[cfg(test)]
#[path = "resources/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "resources/demand_tests.rs"]
mod demand_tests;

#[cfg(test)]
#[path = "resources/reservation_tests.rs"]
mod reservation_tests;
