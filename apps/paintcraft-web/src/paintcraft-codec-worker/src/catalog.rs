//! Actual synchronous catalog entries shared by Wasm and native ABI controls.
use paintcraft_core::types::Result;
pub(crate) fn dds(bytes: &[u8]) -> Result<Vec<u8>> {
    paintcraft_formats::dds::web_export::v6::execute(bytes, env!("CARGO_PKG_VERSION"), &|| Ok(()))
}
pub(crate) fn msp(bytes: &[u8]) -> Result<Vec<u8>> {
    paintcraft_formats::msp::web_export::v6::execute(bytes, env!("CARGO_PKG_VERSION"), &|| Ok(()))
}

#[cfg(test)]
mod tests;
