//! PCND's initial binary format: bounded flat raster documents and inline tiles.
use paintcraft_core::types::policy::STAGED_PIXELS_MAX;
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba};
use paintcraft_document::{COORD_ABS_MAX, Document, LAYER_COUNT_MAX, Layer};
use paintcraft_raster::{Admission, RasterLayer, TILE_EDGE};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub const MAGIC: &[u8; 8] = b"PCND\x01\r\n\x1a";
pub const SUPPORTED_FEATURES: [&str; 3] = ["raster.rgba8", "canvas.finite", "layers.flat"];
const DOCUMENT_MAX: u64 = 8_388_608;
const BLOCK_BYTES_MAX: u64 = 262_144;
mod binary;
mod compression;
mod runs;

/// Deterministic raw encoding for portable conformance and content comparisons.
/// The header and document semantics are identical to normal compressed saves.
pub fn write(doc: &Document) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    write_to(doc, &mut bytes)?;
    Ok(bytes)
}
pub fn write_to(doc: &Document, sink: impl std::io::Write) -> Result<()> {
    binary::write_raw(doc, sink, &|| Ok(()))
}
/// Normal save encoding, selecting the smallest lossless tile representation.
pub fn write_compact(doc: &Document) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    write_compact_to(doc, &mut bytes)?;
    Ok(bytes)
}
pub fn write_compact_to(doc: &Document, sink: impl std::io::Write) -> Result<()> {
    write_compact_to_checked(doc, sink, &|| Ok(()))
}
/// Failed or cancelled output remains private and must not be published as saved.
pub fn write_compact_to_checked(
    doc: &Document,
    sink: impl std::io::Write,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    binary::write(doc, sink, check)
}
fn codec(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn unsupported(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}

/// Version assessment derived from the binary header, without constructing tiles.
#[derive(Clone, Debug)]
pub struct Header {
    pub container_version: i64,
    pub document_schema: String,
    pub required_features: Vec<String>,
}
impl Header {
    pub fn require_supported(&self) -> Result<()> {
        if self.container_version != 1 || self.document_schema != "paintcraft.document/v1" {
            return Err(unsupported("unknown native document version"));
        }
        Ok(())
    }
}
/// Validate framing and integrity before assessing a version. Unknown versions
/// remain explicitly unsupported; they are never opened with current semantics.
pub fn probe(bytes: &[u8]) -> Result<Header> {
    binary::probe(bytes)
}
pub fn read(bytes: &[u8]) -> Result<Document> {
    read_with_limits(bytes, u64::MAX, STAGED_PIXELS_MAX)
}
pub fn read_with_limits(bytes: &[u8], canvas_pixels: u64, staged_pixels: u64) -> Result<Document> {
    read_with_limits_checked(bytes, canvas_pixels, staged_pixels, &|| Ok(()))
}
pub fn read_with_limits_checked(
    bytes: &[u8],
    canvas_pixels: u64,
    staged_pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Document> {
    check()?;
    binary::read(bytes, canvas_pixels, staged_pixels, check)
}
#[cfg(all(test, not(target_arch = "wasm32")))]
mod compact_tests;
#[cfg(test)]
mod tests;
