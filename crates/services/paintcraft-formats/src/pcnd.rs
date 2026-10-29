//! PCND's initial binary format: bounded flat raster documents and inline tiles.
use paintcraft_core::types::policy::STAGED_PIXELS_MAX;
use paintcraft_core::types::resources::SharedBudget;
use paintcraft_core::types::{Error, ErrorCode, Result, Rgba};
use paintcraft_document::{COORD_ABS_MAX, Document, LAYER_COUNT_MAX, Layer};
use paintcraft_raster::{Admission, RasterLayer, TILE_EDGE};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub const MAGIC: &[u8; 8] = b"PCND\x01\r\n\x1a";
pub const SUPPORTED_FEATURES: [&str; 3] = ["raster.rgba8", "canvas.finite", "layers.flat"];
pub const SOURCE_METADATA_FEATURE: &str = "source-metadata.v1";
const DOCUMENT_MAX: u64 = 8_388_608;
const BLOCK_BYTES_MAX: u64 = 262_144;
mod binary;
mod compression;
mod preflight;
mod runs;

/// Check that the current snapshot fits canonical PCND metadata admission.
/// This does not promise future layer/title growth remains within that limit.
pub fn check_metadata_admission(doc: &Document, check: &dyn Fn() -> Result<()>) -> Result<()> {
    preflight::check(doc, check)
}

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
        if !matches!(
            (self.container_version, self.document_schema.as_str()),
            (1, "paintcraft.document/v1") | (2, "paintcraft.document/v2")
        ) {
            return Err(unsupported("unknown native document version"));
        }
        if self.required_features.iter().any(|feature| {
            !SUPPORTED_FEATURES.contains(&feature.as_str())
                && !(self.container_version == 2 && feature == SOURCE_METADATA_FEATURE)
        }) {
            return Err(unsupported("unknown native document required feature"));
        }
        Ok(())
    }
}
/// The attachment byte limit is additional to the existing aggregate PCND
/// metadata ceiling. The caller retains ownership of this budget realm.
#[derive(Clone, Copy, Debug)]
pub struct MetadataAdmission<'a> {
    pub bytes: u64,
    pub budget: &'a SharedBudget,
}
pub fn read_with_metadata_admission_checked(
    bytes: &[u8],
    canvas_pixels: u64,
    staged_pixels: u64,
    layer_limit: usize,
    metadata: MetadataAdmission<'_>,
    check: &dyn Fn() -> Result<()>,
) -> Result<Document> {
    check()?;
    if layer_limit > LAYER_COUNT_MAX {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Invalid host PCND layer limit",
        ));
    }
    binary::read_with_metadata(
        bytes,
        canvas_pixels,
        staged_pixels,
        layer_limit,
        metadata,
        check,
    )
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
/// The existing parser with an additional host-owned layer admission bound.
/// Count admission precedes layer names, records and raster construction; legacy
/// read wrappers retain the document-wide limit rather than inheriting this one.
pub fn read_with_layer_limit_checked(
    bytes: &[u8],
    canvas_pixels: u64,
    staged_pixels: u64,
    layer_limit: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Document> {
    check()?;
    if layer_limit > LAYER_COUNT_MAX {
        return Err(Error::new(
            ErrorCode::InputSchema,
            "Invalid host PCND layer limit",
        ));
    }
    binary::read_with_layer_limit(bytes, canvas_pixels, staged_pixels, layer_limit, check)
}
#[cfg(all(test, not(target_arch = "wasm32")))]
mod compact_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod source_metadata_history_tests;
#[cfg(test)]
mod source_metadata_tests;
#[cfg(test)]
mod tests;
