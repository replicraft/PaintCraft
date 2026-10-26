//! Isolated native AVIF identity decoder and private SDR qualification.
mod execute;
mod ffi;
mod heap;
pub(crate) use execute::execute;
#[cfg(test)]
mod sdr_tests;
