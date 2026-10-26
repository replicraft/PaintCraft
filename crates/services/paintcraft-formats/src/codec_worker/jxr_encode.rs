//! Typed private JXR encode frames. No worker dispatch or production admission.
//!
//! Distinct magic prevents an encoded file from masquerading as decoded RGBA.
//! Caller admits source/output buffers; these pure types do not publish state.
mod request;
mod response;
use super::{HEAP_BYTES, Owner, invalid, resource, unsupported};
use paintcraft_core::types::{Error, ErrorCode, Result};
pub use request::{Header, Request};
pub use response::{EncodedFrame, decode_response, write_failure, write_success};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

pub const REQUEST_MAGIC: &[u8; 8] = b"PCENC001";
pub const RESPONSE_MAGIC: &[u8; 8] = b"PCJXR001";
pub const VERSION: u16 = 1;
pub const REQUEST_HEADER: usize = 176;
pub const RESPONSE_HEADER: usize = 208;
pub const PIXELS_MAX: u64 = 1_048_576;
pub const SIDE_MAX: u32 = 65_536;
pub const RGB_BYTES_MAX: u64 = PIXELS_MAX * 3;
pub const ENCODED_BYTES_MAX: u64 = 64 << 20;
pub const VERSION_BYTES_MAX: usize = 64;
pub const DETAIL_BYTES_MAX: usize = 4096;
pub const PROFILE: &str = "jxr-rgb8-lossless-yuv444-spatial-qp0-no-overlap-zero-dpi-v1";
pub const COLOR_POLICY: &str = crate::jxr::COLOR_POLICY;
pub const DENSITY_POLICY: &str = "t832-stored-positive-zero-resolution-effective-96-dpi";
const PROFILE_TAG: u8 = 1;
const HASH_DOMAIN: &[u8] = b"paintcraft-jxr-rgb8-encode-request/v1\0";
const CHUNK: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    width: u32,
    height: u32,
    stride: u32,
    rgb_bytes: u64,
    encoded_cap: u64,
}
impl Layout {
    /// Packed RGB8 only: callers must explicitly validate/convert any RGBA first.
    /// There are no inferred alpha, quality, depth, color or resolution settings.
    pub fn new(width: u32, height: u32, encoded_cap: u64) -> Result<Self> {
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| resource("JXR encode pixel extent overflow"))?;
        if width == 0
            || height == 0
            || width > SIDE_MAX
            || height > SIDE_MAX
            || pixels > PIXELS_MAX
            || encoded_cap == 0
            || encoded_cap > ENCODED_BYTES_MAX
        {
            return Err(resource("JXR encode layout exceeds private profile"));
        }
        let stride = width
            .checked_mul(3)
            .ok_or_else(|| resource("JXR encode stride overflow"))?;
        let rgb_bytes = u64::from(stride)
            .checked_mul(u64::from(height))
            .ok_or_else(|| resource("JXR encode RGB extent overflow"))?;
        Ok(Self {
            width,
            height,
            stride,
            rgb_bytes,
            encoded_cap,
        })
    }
    pub fn width(self) -> u32 {
        self.width
    }
    pub fn height(self) -> u32 {
        self.height
    }
    pub fn stride(self) -> u32 {
        self.stride
    }
    pub fn rgb_bytes(self) -> u64 {
        self.rgb_bytes
    }
    pub fn encoded_cap(self) -> u64 {
        self.encoded_cap
    }
    pub fn response_limit(self) -> Result<u64> {
        self.encoded_cap
            .checked_add((RESPONSE_HEADER + VERSION_BYTES_MAX + DETAIL_BYTES_MAX) as u64)
            .ok_or_else(|| resource("JXR encode response limit overflow"))
    }
    /// Child ceiling once, plus distinct parent RGB/header/response/IO buffers.
    /// The caller's original document/composite and retained outputs are separate.
    /// Logical admission only: no physical RSS or encoder worst-case claim.
    pub fn reservation_bytes(self) -> Result<u64> {
        HEAP_BYTES
            .checked_add(self.rgb_bytes)
            .and_then(|n| n.checked_add(REQUEST_HEADER as u64))
            .and_then(|n| n.checked_add(self.response_limit().ok()?))
            .and_then(|n| n.checked_add(CHUNK as u64))
            .ok_or_else(|| resource("JXR encode reservation overflow"))
    }
}
fn put_owner(out: &mut [u8], owner: Owner) {
    out[..16].copy_from_slice(owner.session.as_bytes());
    out[16..32].copy_from_slice(owner.state.as_bytes());
    out[32..40].copy_from_slice(&owner.epoch.to_be_bytes());
    out[40] = owner.frontend;
}
fn get_owner(bytes: &[u8]) -> Owner {
    Owner {
        session: uuid::Uuid::from_bytes(bytes[..16].try_into().unwrap()),
        state: uuid::Uuid::from_bytes(bytes[16..32].try_into().unwrap()),
        epoch: u64::from_be_bytes(bytes[32..40].try_into().unwrap()),
        frontend: bytes[40],
    }
}
fn hash(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<[u8; 32]> {
    check()?;
    let mut digest = Sha256::new();
    for chunk in bytes.chunks(CHUNK) {
        check()?;
        digest.update(chunk);
    }
    check()?;
    Ok(digest.finalize().into())
}
fn read_checked(
    reader: &mut impl Read,
    out: &mut [u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    for chunk in out.chunks_mut(CHUNK) {
        let mut offset = 0;
        while offset < chunk.len() {
            check()?;
            match reader.read(&mut chunk[offset..]) {
                Ok(n) if n > 0 && n <= chunk.len() - offset => offset += n,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                _ => return Err(invalid("Truncated JXR encode frame or read failure")),
            }
        }
    }
    check()
}
fn write_checked(
    writer: &mut impl Write,
    bytes: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    for chunk in bytes.chunks(CHUNK) {
        let mut offset = 0;
        while offset < chunk.len() {
            check()?;
            match writer.write(&chunk[offset..]) {
                Ok(n) if n > 0 && n <= chunk.len() - offset => offset += n,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                _ => return Err(invalid("JXR encode frame write failure")),
            }
        }
    }
    check()
}
fn stale() -> Error {
    Error::new(
        ErrorCode::ConflictStaleBase,
        "JXR encode request/source/owner changed",
    )
}

#[cfg(test)]
mod tests;
