//! Explicit generic prepared entry7; old codec entry points stay independent.
use paintcraft_core::types::Result;
pub(crate) fn dds(bytes: &[u8]) -> Result<Vec<u8>> {
    paintcraft_formats::dds::web_export::v7::execute(bytes, env!("CARGO_PKG_VERSION"), &|| Ok(()))
}
pub(crate) fn msp(bytes: &[u8]) -> Result<Vec<u8>> {
    paintcraft_formats::msp::web_export::v7::execute(bytes, env!("CARGO_PKG_VERSION"), &|| Ok(()))
}
#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(test)]
mod tests;
