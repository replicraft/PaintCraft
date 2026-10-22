//! Transfer an upfront reservation into independently owned resource leases.
use super::*;

#[derive(Debug)]
pub(super) struct Reserved {
    pub(super) available: Mutex<u64>,
    capacity: u64,
    budget: SharedBudget,
}
impl Drop for Reserved {
    fn drop(&mut self) {
        let unused = *self
            .available
            .get_mut()
            .unwrap_or_else(|error| error.into_inner());
        // No reservation lock is held while release callbacks run.
        self.budget.release(unused);
    }
}
impl Reserved {
    fn check_available(&self, available: u64, bytes: u64) -> Result<()> {
        if bytes > available {
            let required = (self.capacity - available)
                .checked_add(bytes)
                .ok_or_else(|| {
                    Error::new(ErrorCode::ResourceLimit, "reservation demand overflow")
                })?;
            return Err(Error::denied(self.budget.clone(), required)
                .with_message("operation reservation exhausted"));
        }
        Ok(())
    }
    pub(super) fn take(&self, bytes: u64) -> Result<()> {
        let mut available = self.available.lock().unwrap_or_else(|e| e.into_inner());
        self.check_available(*available, bytes)?;
        *available -= bytes;
        Ok(())
    }
    pub(super) fn recycle(&self, bytes: u64) {
        *self.available.lock().unwrap_or_else(|e| e.into_inner()) += bytes;
    }
}
/// A bounded reservation for one attempt. Clones share the same remaining capacity.
/// Unused bytes return when the last reservation owner drops; surviving leases
/// retain only their own bytes. A live reservation recycles its released leases.
#[derive(Clone, Debug)]
pub struct BudgetReservation(Arc<Reserved>);
impl SharedBudget {
    pub fn reserve_capacity(&self, bytes: u64) -> Result<BudgetReservation> {
        self.admit(bytes)?;
        Ok(BudgetReservation(Arc::new(Reserved {
            available: Mutex::new(bytes),
            capacity: bytes,
            budget: self.clone(),
        })))
    }
}
impl BudgetReservation {
    pub(super) fn downgrade(&self) -> std::sync::Weak<Reserved> {
        Arc::downgrade(&self.0)
    }
    pub fn budget(&self) -> SharedBudget {
        self.0.budget.clone()
    }
    pub fn capacity(&self) -> u64 {
        self.0.capacity
    }
    pub fn available(&self) -> u64 {
        *self
            .0
            .available
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
    /// Advisory validation without allocating a lease or transferring bytes.
    pub fn ensure_capacity(&self, bytes: u64) -> Result<()> {
        self.0.check_available(self.available(), bytes)
    }
    /// Transfer reserved bytes, without competing for the shared realm again.
    /// A request beyond this reservation never borrows unreserved capacity.
    pub fn reserve(&self, bytes: u64) -> Result<BudgetLease> {
        self.0.take(bytes)?;
        Ok(BudgetLease {
            _charge: Arc::new(Charge {
                budget: self.0.budget.clone(),
                bytes,
                reservation: Some(Arc::downgrade(&self.0)),
            }),
        })
    }
}
