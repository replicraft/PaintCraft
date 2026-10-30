//! Layered foreign writers cannot silently replace editable filters with raw pixels.
use paintcraft_core::types::{Error, ErrorCode, Result};
use paintcraft_document::Document;

pub(crate) fn require_unfiltered_layers(document: &Document) -> Result<()> {
    if document
        .layers
        .values()
        .any(|layer| !layer.filters.is_empty())
    {
        return Err(Error::new(
            ErrorCode::CapabilityUnsupported,
            "This layered format cannot preserve editable filters; use a native project or an explicit flat copy",
        ));
    }
    Ok(())
}
