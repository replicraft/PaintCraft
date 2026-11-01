//! Explicit Export8 DDS/MSP endpoints; no JXR route is granted here.
pub(crate) fn dds(bytes: &[u8]) -> paintcraft_core::types::Result<Vec<u8>> {
    paintcraft_formats::dds::web_export::v8::execute(bytes, env!("CARGO_PKG_VERSION"), &|| Ok(()))
}
#[cfg(test)]
mod tests;

/// Actual version-aware MSP8 isolated endpoint; legacyMSP2 remains separate.
pub(crate) fn msp(bytes: &[u8]) -> paintcraft_core::types::Result<Vec<u8>> {
    paintcraft_formats::msp::web_export::v8::execute(bytes, env!("CARGO_PKG_VERSION"), &|| Ok(()))
}
#[cfg(test)]
mod msp_tests;
