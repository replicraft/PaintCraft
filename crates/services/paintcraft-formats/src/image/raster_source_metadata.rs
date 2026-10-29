//! Bounded source facts, not retained document metadata or color management.
use paintcraft_core::types::{Error, ErrorCode, Result};
use sha2::{Digest as _, Sha256};
use std::io::Read;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Digest {
    pub bytes: u64,
    pub sha256: [u8; 32],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Icc {
    pub payload: Digest,
    pub color_space: [u8; 4],
    pub connection_space: [u8; 4],
    pub tag_count: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolutionUnit {
    Unknown,
    Meter,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub x: u32,
    pub y: u32,
    pub unit: ResolutionUnit,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Palette {
    pub rgb: Vec<[u8; 3]>,
    pub alpha: Vec<u8>,
}
pub(super) fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::CodecFailed, message)
}
pub(super) fn unsupported(message: &str) -> Error {
    Error::new(ErrorCode::CapabilityUnsupported, message)
}
pub(super) fn resource(message: &str) -> Error {
    Error::new(ErrorCode::ResourceLimit, message)
}
pub(super) fn allowance(bytes: &[u8], pixels: u64) -> Result<u64> {
    if bytes.len() as u64 > 1 << 30 || bytes.len() > isize::MAX as usize || pixels == 0 {
        return Err(resource("Raster metadata source/pixel allowance"));
    }
    Ok(pixels
        .min(super::DECODE_PIXELS_MAX)
        .saturating_mul(8)
        .min(isize::MAX as u64))
}
pub(super) fn hash_update(
    hash: &mut Sha256,
    bytes: &[u8],
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    for chunk in bytes.chunks(65536) {
        check()?;
        hash.update(chunk);
    }
    check()
}
pub(super) fn digest(bytes: &[u8], check: &dyn Fn() -> Result<()>) -> Result<Digest> {
    let mut hash = Sha256::new();
    hash_update(&mut hash, bytes, check)?;
    Ok(Digest {
        bytes: bytes.len() as u64,
        sha256: hash.finalize().into(),
    })
}
/// Validate only ICC container bounds and header/tag-table structure. No tag
/// interpretation, profile suitability or color conversion is implied.
pub(super) fn inspect_icc(
    mut input: impl Read,
    limit: u64,
    check: &dyn Fn() -> Result<()>,
) -> Result<Icc> {
    check()?;
    let mut header = [0u8; 132];
    input
        .read_exact(&mut header)
        .map_err(|_| invalid("Truncated ICC header"))?;
    let size = u32::from_be_bytes(header[..4].try_into().unwrap());
    let tags = u32::from_be_bytes(header[128..132].try_into().unwrap());
    let table_end = 132u64
        .checked_add(u64::from(tags) * 12)
        .ok_or_else(|| invalid("ICC tag table extent"))?;
    if u64::from(size) > limit {
        return Err(resource("ICC inspection allowance"));
    }
    if size < 132 || table_end > u64::from(size) || &header[36..40] != b"acsp" {
        return Err(invalid("Malformed ICC header/tag table"));
    }
    let mut hash = Sha256::new();
    hash.update(header);
    for _ in 0..tags {
        check()?;
        let mut entry = [0u8; 12];
        input
            .read_exact(&mut entry)
            .map_err(|_| invalid("Truncated ICC tag table"))?;
        let offset = u64::from(u32::from_be_bytes(entry[4..8].try_into().unwrap()));
        let length = u64::from(u32::from_be_bytes(entry[8..12].try_into().unwrap()));
        if offset < table_end
            || offset
                .checked_add(length)
                .is_none_or(|end| end > u64::from(size))
        {
            return Err(invalid("ICC tag exceeds profile payload"));
        }
        hash.update(entry);
    }
    let mut total = table_end;
    let mut buffer = [0u8; 65536];
    loop {
        check()?;
        let count = input
            .read(&mut buffer)
            .map_err(|_| invalid("Malformed ICC compression/payload"))?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| resource("ICC payload extent"))?;
        if total > limit {
            return Err(resource("ICC inspection allowance"));
        }
        if total > u64::from(size) {
            return Err(invalid("ICC payload exceeds declared size"));
        }
        hash.update(&buffer[..count]);
    }
    if total != u64::from(size) {
        return Err(invalid("Truncated ICC profile payload"));
    }
    check()?;
    Ok(Icc {
        payload: Digest {
            bytes: total,
            sha256: hash.finalize().into(),
        },
        color_space: header[16..20].try_into().unwrap(),
        connection_space: header[20..24].try_into().unwrap(),
        tag_count: tags,
    })
}
