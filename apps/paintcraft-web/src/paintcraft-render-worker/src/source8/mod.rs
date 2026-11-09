//! Explicit Rust-only closed portable Source8 scalar inspection.
//! Existing Worker/JS protocols and ordinary import routes remain unchanged.
use paintcraft_core::types::{
    Error, ErrorCode, Result,
    resources::{BudgetAccount, BudgetReservation, BudgetScope},
};
mod adoption;
mod assembly_ledger;
mod authenticate;
mod build;
mod caller;
mod entry;
mod inputs;
mod inspection;
mod metadata;
mod owner;
mod owner_ledger;
mod probe_ledger;
pub use inputs::{
    Source8InspectionMode, Source8InspectionRequest, Source8ReadCaps, Source8ReadRealms,
    Source8ReadScopes,
};
pub use inspection::{Source8Inspection, inspect_portable_source8_checked};
fn extent() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Source8 read ownership extent")
}
fn incomplete() -> Error {
    Error::new(
        ErrorCode::InvariantViolation,
        "Source8 read scalar inspection is incomplete",
    )
}
#[cfg(test)]
mod tests;
