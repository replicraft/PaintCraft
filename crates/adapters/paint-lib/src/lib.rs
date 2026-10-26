//! Bounded ISO box inspection and the observed layered PAINT item profile.
//!
//! An ISO/HEIF brand does not identify a Microsoft Paint project. Its item
//! graph must be validated before pixels are decoded. The scoped layered decoder
//! exposes unresolved layer-property diagnostics; no preserving writer is claimed.
mod boxes;
mod decode;
pub use decode::{DecodeLimits, Decoded, DecodedLayer, Diagnostic, decode_layers};
mod extents;
mod write;
pub use boxes::{BoxInfo, BoxTree, FileType, LayoutResolver, inspect, inspect_with_layout};
pub use extents::{AliasPolicy, Extent, resolve_extent, validate_extents};
use paintcraft_core::types::{Error, ErrorCode, Result};
pub use write::{
    WriteDocument, WriteLayer, WriteLimits, WritePlan, encode_document, preflight_write,
    write_document_to,
};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub source_bytes: usize,
    pub boxes: usize,
    pub depth: usize,
    pub metadata_bytes: usize,
    pub compatible_brands: usize,
    pub extents: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            source_bytes: 1 << 30,
            boxes: 16_384,
            depth: 64,
            metadata_bytes: 8 << 20,
            compatible_brands: 64,
            extents: 16_384,
        }
    }
}
fn malformed(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}

/// Only enumerate boxes inside containers whose prefix has been verified by
/// the caller. A type can have different semantics depending on its ancestry.
/// The built-in structural slice knows meta-v0, iprp and ipco; all other
/// payloads are opaque. No arbitrary unknown bytes are interpreted as boxes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Opaque,
    Children { prefix_bytes: usize },
}
#[cfg(test)]
mod tests;
