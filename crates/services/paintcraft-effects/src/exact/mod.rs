//! Effects-private fixed integers shared by independently specified compilers.
//! No heap arithmetic, unsafe casts, runtime backend selection or global state.
use paintcraft_core::types::{Error, Result};
pub(crate) mod integer;
pub(crate) mod word;
#[derive(Clone, Copy)]
pub(crate) enum Fault {
    ScalarPower,
    IntegerCapacity,
    ProductCapacity,
}
/// Monomorphized accounting/error context. Each caller defines its own units,
/// limits, callbacks and diagnostics; sharing storage does not share authority.
pub(crate) trait Budget {
    fn charge(&mut self, units: u64) -> Result<()>;
    fn resource(&self, fault: Fault) -> Error;
}
