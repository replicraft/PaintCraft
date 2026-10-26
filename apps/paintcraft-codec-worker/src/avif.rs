//! Isolated native AVIF identity decoder and private SDR qualification.
mod execute;
mod ffi;
mod heap;
pub(crate) use execute::execute;
#[cfg(test)]
mod sdr_tests;

#[cfg(feature = "avif-sdr-codec")]
mod sdr_execute;
#[cfg(feature = "avif-sdr-codec")]
pub(crate) use sdr_execute::execute as execute_sdr;
