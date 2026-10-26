//! Bounded Radiance RGBE header/packet admission, never an enabled decoder.
//! Stored values already contain header exposure/correction adjustments; no
//! metadata is silently applied or undone by inspection.
mod header;
mod packets;
pub use header::{Header, Record};
pub use packets::Summary;
use paintcraft_core::types::{Error, ErrorCode, Result};
use sha2::{Digest, Sha256};
pub const INPUT_BYTES_MAX: usize = 64 << 20;
pub const HEADER_BYTES_MAX: usize = 64 << 10;
pub const LINE_BYTES_MAX: usize = 4096;
pub const HEADER_LINES_MAX: usize = 256;
pub const PIXELS_MAX: u64 = 4_194_304;
#[derive(Debug)]
pub struct Inspection<'a> {
    header: Header<'a>,
    packets: Summary,
    sha256: [u8; 32],
}
impl Inspection<'_> {
    pub fn header(&self) -> &Header<'_> {
        &self.header
    }
    pub fn packets(&self) -> Summary {
        self.packets
    }
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
}
fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
/// Source bytes must already belong to caller admission. All header records are
/// borrowed; bounded packet validation allocates no pixel/scanline storage.
/// Work is bounded by source bytes plus at most five packets per admitted pixel.
pub fn inspect<'a>(
    bytes: &'a [u8],
    pixels: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Inspection<'a>> {
    check()?;
    if bytes.len() > INPUT_BYTES_MAX {
        return Err(resource("Radiance source exceeds byte allowance"));
    }
    let header = header::parse(bytes, pixels, check)?;
    let packets = packets::validate(
        &bytes[header.data_offset..],
        header.width,
        header.height,
        check,
    )?;
    let mut hash = Sha256::new();
    for chunk in bytes.chunks(64 << 10) {
        check()?;
        hash.update(chunk);
    }
    check()?;
    Ok(Inspection {
        header,
        packets,
        sha256: hash.finalize().into(),
    })
}

#[cfg(test)]
mod tests;
