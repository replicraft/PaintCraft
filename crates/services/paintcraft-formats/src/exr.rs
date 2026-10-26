//! Private bounded EXR admission only. No decoder or public reader enabled.
mod channels;
mod chunks;
mod header;
pub use channels::{Channel, SampleType};
pub use header::{Attribute, Header};
use paintcraft_core::types::{Error, ErrorCode, Result};
use sha2::{Digest, Sha256};
pub const INPUT_BYTES_MAX: usize = 64 << 20;
pub const HEADER_BYTES_MAX: usize = 64 << 10;
pub const ATTRIBUTES_MAX: usize = 128;
pub const PIXELS_MAX: u64 = 4_194_304;
#[derive(Debug)]
pub struct Inspection<'a> {
    header: Header<'a>,
    source: &'a [u8],
    sha256: [u8; 32],
    chunks: u32,
}
impl Inspection<'_> {
    pub fn header(&self) -> &Header<'_> {
        &self.header
    }
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
    pub fn source_len(&self) -> usize {
        self.source.len()
    }
    pub fn chunk_count(&self) -> u32 {
        self.chunks
    }
}
fn invalid(s: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, s)
}
fn unsupported(s: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, s)
}
fn resource(s: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, s)
}
struct Bytes<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Bytes<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(n)
            .ok_or_else(|| resource("EXR byte extent overflow"))?;
        let out = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| invalid("Truncated EXR field"))?;
        self.at = end;
        Ok(out)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn text(&mut self) -> Result<&'a str> {
        let tail = self
            .bytes
            .get(self.at..)
            .ok_or_else(|| invalid("Truncated EXR name"))?;
        let size = tail
            .iter()
            .take(32)
            .position(|b| *b == 0)
            .ok_or_else(|| unsupported("EXR long names exceed initial profile"))?;
        let out = std::str::from_utf8(self.take(size)?)
            .map_err(|_| unsupported("EXR non-UTF8 names unqualified"))?;
        self.take(1)?;
        Ok(out)
    }
}
/// Callers admit source storage before inspection; no codec parses untrusted
/// metadata here. The sealed source includes original offsets and provenance.
pub fn inspect<'a>(
    source: &'a [u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Inspection<'a>> {
    check()?;
    if source.len() > INPUT_BYTES_MAX {
        return Err(resource("EXR source exceeds allowance"));
    }
    let mut bytes = Bytes {
        bytes: source,
        at: 0,
    };
    if bytes.u32()? != 20_000_630 {
        return Err(invalid("EXR magic mismatch"));
    }
    if bytes.u32()? != 2 {
        return Err(unsupported(
            "Initial EXR profile requires v2 single-part scanlines, short names, no deep/tiled data",
        ));
    }
    let header = header::parse(&mut bytes, pixels, check)?;
    let chunks = chunks::validate(&mut bytes, &header, check)?;
    let mut hash = Sha256::new();
    for part in source.chunks(64 << 10) {
        check()?;
        hash.update(part);
    }
    check()?;
    Ok(Inspection {
        header,
        source,
        sha256: hash.finalize().into(),
        chunks,
    })
}
#[cfg(test)]
mod tests;
