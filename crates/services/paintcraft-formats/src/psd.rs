//! Preflighted PSD/PSB RGB8 raster-layer conversion through the selected ag-psd.
//! Advanced Photoshop semantics and foreign preserving saves are separate gates.
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::ops::Range;
mod channels;
mod decode;
mod scan;
mod write;
pub use decode::{Decoded, DecodedLayer, decode};
pub use write::{
    TITLE_LOSS, WriteLimits, WritePlan, encode_document, preflight, write_document_to,
};
#[cfg(test)]
mod tests;

pub const ENCODED_MAX: usize = 64 << 20;
pub const PIXELS_MAX: u64 = 4_194_304;
pub const LAYERS_MAX: usize = 256;
/// Shared PSD reader/fresh-writer allowance in UTF-16 code units.
pub const LAYER_NAME_UTF16_MAX: usize = 1024;
const METADATA_MAX: usize = 1 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    Psd,
    Psb,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerInfo {
    pub name: String,
    pub origin: (i32, i32),
    pub width: u32,
    pub height: u32,
    pub opacity: u8,
    pub visible: bool,
    pub source_id: Option<u32>,
    channels: Vec<(i16, Range<usize>)>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspection {
    pub version: Version,
    pub width: u32,
    pub height: u32,
    /// Source file order, background to foreground.
    pub layers: Vec<LayerInfo>,
    pub losses: Vec<String>,
    resources: Range<usize>,
    composite: usize,
}
fn malformed(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
fn loss(losses: &mut Vec<String>, code: &str) {
    if !losses.iter().any(|known| known == code) {
        losses.push(code.into());
    }
}

/// Inspect structure and validate the exact decoded length of every compressed
/// channel before any selected-library decoding. No pixel buffers are retained.
pub fn inspect(
    bytes: &[u8],
    pixel_limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Inspection> {
    scan::inspect(bytes, pixel_limit.min(PIXELS_MAX), check)
}

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    fn remaining(&self) -> usize {
        self.bytes.len() - self.pos
    }
    fn take(&mut self, size: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(size)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| malformed("PSD section exceeds its declared boundary"))?;
        let bytes = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(bytes)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn length(&mut self, large: bool) -> Result<usize> {
        let length = if large {
            u64::from_be_bytes(self.take(8)?.try_into().unwrap())
        } else {
            u64::from(self.u32()?)
        };
        usize::try_from(length).map_err(|_| resource("PSD length exceeds target address space"))
    }
    fn section(&mut self, large: bool) -> Result<&'a [u8]> {
        let size = self.length(large)?;
        self.take(size)
    }
    fn zeros(&mut self, size: usize) -> Result<()> {
        if self.take(size)?.iter().any(|value| *value != 0) {
            return Err(malformed(
                "PSD reserved bytes or alignment padding are nonzero",
            ));
        }
        Ok(())
    }
}
