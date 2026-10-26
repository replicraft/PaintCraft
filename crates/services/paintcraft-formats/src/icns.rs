//! Bounded modern PNG ICNS resources, distinct logical size and Retina density.
//! Internal backend qualification; no registry/consumer activation yet.
mod read;
#[cfg(test)]
mod tests;
mod types;
mod write;
use paintcraft_core::types::{Error, ErrorCode, Result};
pub use read::{decode_entry, inspect};
pub use types::IconType;
pub use write::{WriteEntry, encode_entries};
pub const BYTES_MAX: usize = 64 << 20;
pub const ENTRY_BYTES_MAX: usize = 8 << 20;
pub const OPAQUE_BYTES_MAX: usize = 256 << 10;
pub const RECORDS_MAX: usize = 64;
pub const PIXELS_MAX: u64 = 4_194_304;
pub const KNOWN_IMPORT_LOSSES: &[&str] = &[
    "icon-other-entries-not-imported",
    "icon-palette-expanded",
    "icns-metadata-ignored",
    "icon-density-not-retained-in-flat-document",
    "exif-metadata-ignored",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Png,
    Unqualified,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub tag: [u8; 4],
    pub offset: u64,
    pub length: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub index: u16,
    pub record: u16,
    pub tag: [u8; 4],
    pub logical_width: u32,
    pub logical_height: u32,
    pub scale: u32,
    pub width: u32,
    pub height: u32,
    pub encoding: Encoding,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Directory {
    pub encoded_length: u64,
    pub records: Vec<Record>,
    pub entries: Vec<Entry>,
}
#[derive(Debug)]
pub struct Decoded {
    pub entry: Entry,
    pub rgba: Vec<u8>,
    pub losses: Vec<String>,
}
impl Decoded {
    /// Explicit selected record identity, even for duplicate types/pixel sizes.
    pub fn document(&self, source_sha256: &str) -> Result<paintcraft_document::Document> {
        paintcraft_document::import_image(
            self.entry.width,
            self.entry.height,
            &self.rgba,
            &format!(
                "{source_sha256}:icns-entry:{}:{}",
                self.entry.index, self.entry.record
            ),
        )
    }
}
fn bad(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn limit(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
fn allocation<T>(count: usize) -> Result<Vec<T>> {
    let mut value = Vec::new();
    value
        .try_reserve_exact(count)
        .map_err(|_| limit("ICNS allocation failed"))?;
    Ok(value)
}
