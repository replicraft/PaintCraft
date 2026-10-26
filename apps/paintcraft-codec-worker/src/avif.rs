//! Private native AVIF decoder; no public import gate is enabled.
mod execute;
mod ffi;
mod heap;
pub(crate) use execute::execute;
