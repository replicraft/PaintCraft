//! Actual Wasm Worker binding. It shares decoder/protocol implementation with
//! the native helper; it does not initialize editor, GPU or application services.
#[cfg(target_arch = "wasm32")]
mod wasm;
