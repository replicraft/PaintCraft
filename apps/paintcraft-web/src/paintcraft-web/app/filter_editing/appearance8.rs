//! Genuine committed Source8 Main lane. Legacy4–7 remain separate consumers.
#[cfg(target_arch = "wasm32")]
mod bridge;
mod capture;
mod controls;
#[cfg(target_arch = "wasm32")]
pub(super) mod display;
mod factory;
mod frame;
mod future;
#[cfg(target_arch = "wasm32")]
mod js;
mod operation;
#[cfg(target_arch = "wasm32")]
mod owner;
mod publication;
mod snapshot;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "appearance8/tests.rs"]
mod tests;
fn denied() -> paintcraft_core::types::Error {
    paintcraft_core::types::Error::new(
        paintcraft_core::types::ErrorCode::ResourceLimit,
        "Source8 Main original ownership or phase limit",
    )
}

#[cfg(target_arch = "wasm32")]
fn stale() -> paintcraft_core::types::Error {
    paintcraft_core::types::Error::new(
        paintcraft_core::types::ErrorCode::ConflictStaleBase,
        "Source8 Main source or view changed",
    )
}
