//! Windows pointer history and canonical mouse acquisition for an owned loop.
//! Source/state controls compile headlessly; actual Win32 hooks are OS-scoped.
mod batch;
mod clock;
mod cutoff;
mod model;
mod resident;
mod touch;
mod transform;

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod ffi;
#[cfg(target_os = "windows")]
pub use ffi::{Hook, Installation};

#[cfg(test)]
mod tests;
