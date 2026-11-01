//! Actual Export8 DDS endpoint; no MSP/JXR route is granted here.
pub(crate) fn dds(bytes: &[u8]) -> paintcraft_core::types::Result<Vec<u8>> {
    paintcraft_formats::dds::web_export::v8::execute(bytes, env!("CARGO_PKG_VERSION"), &|| Ok(()))
}
#[cfg(test)]
mod tests;
