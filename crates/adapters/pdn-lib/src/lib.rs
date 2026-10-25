//! Historical PDN3 envelope inspection into inert, borrowed DTOs.
//!
//! This is PDN-01 only: header hints are not verified document properties.
//! Object graphs, compressed data, pixels and writers are not implemented or
//! advertised. No .NET objects, constructors or assemblies are ever executed.
mod body;
mod xml;
use paintcraft_core::types::{Error, ErrorCode, Result};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Probe {
    Pdn3,
    NeedMoreInput { additional: usize },
    UnsupportedVariant([u8; 4]),
    NotPdn,
}
pub fn probe(bytes: &[u8]) -> Probe {
    if bytes.len() < 4 {
        return if b"PDN3".starts_with(bytes) {
            Probe::NeedMoreInput {
                additional: 4 - bytes.len(),
            }
        } else {
            Probe::NotPdn
        };
    }
    if bytes.starts_with(b"PDN3") {
        Probe::Pdn3
    } else if bytes.starts_with(b"PDN") {
        Probe::UnsupportedVariant(bytes[..4].try_into().unwrap())
    } else {
        Probe::NotPdn
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub source_bytes: usize,
    pub header_bytes: usize,
    pub xml_depth: usize,
    pub xml_elements: usize,
    pub attributes_per_element: usize,
    pub attribute_bytes: usize,
    pub dimension: u32,
    pub pixels: u64,
    pub layers: u32,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            source_bytes: 1 << 30,
            header_bytes: 4 << 20,
            xml_depth: 64,
            xml_elements: 16_384,
            attributes_per_element: 64,
            attribute_bytes: 1 << 20,
            dimension: 65_536,
            pixels: 268_435_456,
            layers: 1024,
        }
    }
}
/// Claims read from the XML envelope only. Compare these with the eventual
/// decoded graph before using them to create any document or allocate pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderHints {
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub saved_with_version: Option<String>,
    pub contains_child_elements: bool,
    pub contains_unknown_root_attributes: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NrbfHeader {
    pub root_id: i32,
    pub header_id: i32,
    pub major_version: i32,
    pub minor_version: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyHeader {
    DeferredNrbf(NrbfHeader),
    /// Only framing was inspected. DEFLATE, CRC, sizes and NRBF remain unverified.
    Gzip {
        flags: u8,
        header_bytes: usize,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspection<'a> {
    pub hints: HeaderHints,
    pub xml: &'a str,
    pub xml_range: Range<usize>,
    pub body_header: BodyHeader,
    /// Includes the PDN deferred marker or the complete gzip member framing.
    pub body: &'a [u8],
    pub body_offset: usize,
}
pub fn inspect<'a>(
    bytes: &'a [u8],
    limits: &Limits,
    check: &dyn Fn() -> Result<()>,
) -> Result<Inspection<'a>> {
    check()?;
    if bytes.len() > limits.source_bytes {
        return Err(resource("PDN source exceeds the byte limit"));
    }
    match probe(bytes) {
        Probe::Pdn3 => {}
        Probe::UnsupportedVariant(_) => {
            return Err(unsupported("Unsupported PDN signature variant"));
        }
        Probe::NeedMoreInput { .. } => return Err(malformed("Truncated PDN signature")),
        Probe::NotPdn => return Err(malformed("Invalid PDN signature")),
    }
    let length = bytes
        .get(4..7)
        .ok_or_else(|| malformed("Truncated PDN XML length"))?;
    let length =
        usize::from(length[0]) | (usize::from(length[1]) << 8) | (usize::from(length[2]) << 16);
    if length == 0 {
        return Err(malformed("Empty PDN XML header"));
    }
    if length > limits.header_bytes {
        return Err(resource("PDN XML header exceeds the byte limit"));
    }
    let end = 7usize
        .checked_add(length)
        .ok_or_else(|| resource("PDN XML header length overflow"))?;
    let xml = bytes
        .get(7..end)
        .ok_or_else(|| malformed("Truncated PDN XML header"))?;
    let xml = std::str::from_utf8(xml).map_err(|_| malformed("PDN XML must be UTF-8"))?;
    let hints = xml::inspect(xml, limits, check)?;
    let body = bytes
        .get(end..)
        .ok_or_else(|| malformed("Truncated PDN body marker"))?;
    let body_header = body::inspect(body, limits, check)?;
    check()?;
    Ok(Inspection {
        hints,
        xml,
        xml_range: 7..end,
        body_header,
        body,
        body_offset: end,
    })
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
#[cfg(test)]
mod tests;
