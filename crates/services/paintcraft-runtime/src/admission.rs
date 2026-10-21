//! One authority for caller-declared transient bytes across worker pools.
//! These charges are not allocator interception or process RSS accounting.
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::sync::{Arc, Mutex};

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
}
#[derive(Clone, Debug)]
pub struct SharedBudget(Arc<Mutex<State>>);
impl SharedBudget {
    pub fn new(limit: u64) -> Self {
        Self(Arc::new(Mutex::new(State {
            limit,
            reserved: 0,
            peak: 0,
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
    pub(crate) fn can_reserve(&self, bytes: u64) -> bool {
        let usage = self.usage();
        bytes <= usage.limit - usage.reserved
    }
    pub(crate) fn reserve(&self, bytes: u64) -> Result<Lease> {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let reserved = state
            .reserved
            .checked_add(bytes)
            .filter(|total| *total <= state.limit)
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::ResourceLimit,
                    "shared worker byte budget is full",
                )
            })?;
        state.reserved = reserved;
        state.peak = state.peak.max(reserved);
        Ok(Lease {
            _charge: Arc::new(Charge {
                budget: self.clone(),
                bytes,
            }),
        })
    }
}
#[derive(Debug)]
struct Charge {
    budget: SharedBudget,
    bytes: u64,
}
impl Drop for Charge {
    fn drop(&mut self) {
        let mut state = self
            .budget
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        state.reserved -= self.bytes;
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Lease {
    _charge: Arc<Charge>,
}
