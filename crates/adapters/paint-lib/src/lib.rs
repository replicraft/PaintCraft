//! PAINT-01: bounded ISO box structure and byte-extent inspection.
//!
//! An ISO/HEIF brand does not identify a Microsoft Paint project. No item
//! graph, layer pixels, .paint decoder or writer is claimed by this module.
mod boxes;
mod extents;
pub use boxes::{BoxInfo, BoxTree, FileType, LayoutResolver, inspect, inspect_with_layout};
pub use extents::{AliasPolicy, Extent, resolve_extent, validate_extents};
use paintcraft_core::types::{Error, ErrorCode, Result};

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
