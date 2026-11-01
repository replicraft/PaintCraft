//! Separate Source7 consumer; legacy protocol flags/bodies remain independently dispatched.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
#[cfg(target_arch = "wasm32")]
mod bridge;
mod candidate;
mod capture;
mod control;
mod echo;
mod frames;
#[cfg(any(target_arch = "wasm32", test))]
mod joined;
mod key;
mod leases;
mod operation;
mod parent;
mod preparation;
mod prepare;
mod preplan;
mod projection;
mod runtime;
mod snapshot;
#[cfg(target_arch = "wasm32")]
pub(super) use bridge::{CancelSlot7, Client7, CopyControl7};
pub(super) use key::{Key7, Public7};
#[cfg(target_arch = "wasm32")]
pub(super) async fn open(
    key: Key7,
    cancellation: CancelSlot7,
    admission: paintcraft_raster::Admission,
    cancelled: std::rc::Rc<std::cell::Cell<bool>>,
) -> paintcraft_core::types::Result<Client7> {
    let check = || {
        if cancelled.get() {
            Err(paintcraft_core::types::Error::new(
                paintcraft_core::types::ErrorCode::OperationCancelled,
                "Source7 opening cancelled",
            ))
        } else {
            Ok(())
        }
    };
    let (capture, prefix) = key.prepare_capture(&check)?;
    bridge::open(
        capture,
        prefix,
        admission,
        cancellation,
        cancelled,
        key.copy_spec,
    )
    .await
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod binding_tests;
