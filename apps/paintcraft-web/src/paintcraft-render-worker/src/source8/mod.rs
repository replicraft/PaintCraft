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
mod read_controls;
mod service_bridge;
pub(crate) mod service_endpoint;
pub use inputs::{
    Source8InspectionMode, Source8InspectionRequest, Source8ReadCaps, Source8ReadRealms,
    Source8ReadScopes,
};
pub use inspection::{Source8Inspection, inspect_portable_source8_checked};
pub use read_controls::{Source8ReaderControlBytes, source8_reader_control_bytes};
fn extent() -> Error {
    Error::new(ErrorCode::ResourceLimit, "Source8 read ownership extent")
}
fn incomplete() -> Error {
    Error::new(
        ErrorCode::InvariantViolation,
        "Source8 read scalar inspection is incomplete",
    )
}
mod frame_image;
mod frame_inputs;
mod frame_output;
mod frame_public;
mod frame_work;
pub use frame_image::Source8RgbaImage;
pub use frame_inputs::{Source8FrameCaps, Source8FrameRealms, Source8FrameScopes};
pub use frame_public::render_portable_source8_checked;

mod display_public;
pub use display_public::{Source8DisplayPatch, render_portable_source8_display_checked};

#[cfg(test)]
mod tests;
